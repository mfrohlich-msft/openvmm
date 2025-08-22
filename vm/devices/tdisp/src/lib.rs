// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//!
//! WARNING: *** This crate is a work in progress, do not use in production! ***
//!
//! TDISP is a standardized interface for end-to-end encryption and attestation
//! of trusted assigned devices to confidential/isolated partitions. This crate
//! implements structures and interfaces for the host and guest to prepare and
//! assign trusted devices. Examples of technologies that implement TDISP
//! include:
//! - Intel® "TDX Connect"
//! - AMD SEV-TIO

// [TDISP TODO] Remove this once the TDISP interface is stable.
#![allow(dead_code)]

pub mod command;
pub mod serialize;
use std::sync::Arc;

use anyhow::Context;
pub use command::{
    GuestToHostCommand, GuestToHostResponse, TdispCommandId, TdispCommandResponsePayload,
    TdispDeviceInterfaceInfo,
};
use inspect::Inspect;
use parking_lot::Mutex;
use thiserror::Error;

use crate::command::TdispCommandRequestPayload;

/// Major version of the TDISP guest-to-host interface.
pub const TDISP_INTERFACE_VERSION_MAJOR: u32 = 1;

/// Minor version of the TDISP guest-to-host interface.
pub const TDISP_INTERFACE_VERSION_MINOR: u32 = 0;

/// Callback for receiving TDISP commands from the guest.
pub type TdispCommandCallback = dyn Fn(&GuestToHostCommand) -> anyhow::Result<()> + Send + Sync;

/// Trait used by the emulator to call back into the host.
pub trait TdispHostDeviceInterface: Send + Sync {
    /// Bind a tdi device to the current partition.
    fn tdisp_bind(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    /// Unbind a tdi device from the current partition.
    fn tdisp_unbind(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Trait added to host VPCI devices to allow them to dispatch TDISP commands from guests.
pub trait TdispHostDeviceTarget: Send + Sync {
    /// [TDISP TODO] Highly subject to change as we work out the traits and semantics.
    fn tdisp_handle_guest_command(
        &mut self,
        _command: GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse> {
        tracing::warn!("TdispHostDeviceTarget not implemented: tdisp_dispatch");
        anyhow::bail!("TdispHostDeviceTarget not implemented: tdisp_dispatch")
    }
}

/// An emulator which runs the TDISP state machine for a synthetic device.
pub struct TdispHostDeviceTargetEmulator {
    machine: TdispHostStateMachine,
    debug_device_id: String,
}

impl TdispHostDeviceTargetEmulator {
    /// Create a new emulator which runs the TDISP state machine for a synthetic device.
    pub fn new(host_interface: Arc<Mutex<dyn TdispHostDeviceInterface>>) -> Self {
        Self {
            machine: TdispHostStateMachine::new(host_interface),
            debug_device_id: "".to_owned(),
        }
    }

    /// Set the debug device ID string.
    pub fn set_debug_device_id(&mut self, debug_device_id: &str) {
        self.machine.set_debug_device_id(debug_device_id.to_owned());
        self.debug_device_id = debug_device_id.to_owned();
    }

    /// Print a debug message to the log.
    /// [TDISP TODO] Fix print type, make this accept a formatter instead.
    fn debug_print(&self, msg: String) {
        self.machine.debug_print(&msg);
    }

    /// Print an error message to the log.
    fn error_print(&self, msg: String) {
        self.machine.error_print(&msg);
    }

    /// Reset the emulator.
    pub fn reset(&self) {}

    /// Get the device interface info for this device.
    fn get_device_interface_info(&self) -> TdispDeviceInterfaceInfo {
        TdispDeviceInterfaceInfo {
            interface_version_major: TDISP_INTERFACE_VERSION_MAJOR,
            interface_version_minor: TDISP_INTERFACE_VERSION_MINOR,
            supported_features: 0,
            tdisp_device_id: 0,
        }
    }
}

impl TdispHostDeviceTarget for TdispHostDeviceTargetEmulator {
    fn tdisp_handle_guest_command(
        &mut self,
        command: GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse> {
        self.debug_print(format!(
            "tdisp_handle_guest_command: command = {:?}",
            command
        ));

        let mut error = TdispGuestOperationError::Success;
        let mut payload = TdispCommandResponsePayload::None;
        let state_before = self.machine.state();
        match command.command_id {
            TdispCommandId::GetDeviceInterfaceInfo => {
                let interface_info = self.get_device_interface_info();
                payload = TdispCommandResponsePayload::GetDeviceInterfaceInfo(interface_info);
            }
            TdispCommandId::Bind => {
                let bind_res = self.machine.request_lock_device_resources();
                if let Err(err) = bind_res {
                    error = err;
                } else {
                    payload = TdispCommandResponsePayload::None;
                }
            }
            TdispCommandId::Unbind => {
                let unbind_reason: TdispGuestUnbindReason = match command.payload {
                    TdispCommandRequestPayload::Unbind(payload) => payload.unbind_reason.into(),
                    _ => TdispGuestUnbindReason::Unknown,
                };
                let unbind_res = self.machine.request_unbind(unbind_reason);
                if let Err(err) = unbind_res {
                    error = err;
                }
            }
            TdispCommandId::GetTdiReport | TdispCommandId::StartTdi => {
                error = TdispGuestOperationError::NotImplemented;
            }
            TdispCommandId::Unknown => {
                error = TdispGuestOperationError::InvalidGuestCommandId;
            }
        }
        let state_after = self.machine.state();

        match error {
            TdispGuestOperationError::Success => {
                self.debug_print("tdisp_handle_guest_command: Success".to_owned());
            }
            _ => {
                self.error_print(format!("tdisp_handle_guest_command: Error: {:?}", error));
            }
        }

        let resp = GuestToHostResponse {
            command_id: command.command_id,
            result: error,
            tdi_state_before: state_before,
            tdi_state_after: state_after,
            payload,
        };

        self.debug_print(format!("tdisp_handle_guest_command: response = {:?}", resp));

        Ok(resp)
    }
}

/// Trait implemented by TDISP-capable devices on the client side. This includes devices that
/// are assigned to isolated partitions other than the host.
pub trait TdispClientDevice: Send + Sync {
    /// Send a TDISP command to the host for this device.
    /// [TDISP TODO] Async? Better handling of device_id in GuestToHostCommand?
    fn tdisp_command_to_host(&self, command: GuestToHostCommand) -> anyhow::Result<()>;
}

/// Represents the state of the TDISP host device emulator.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Inspect)]
pub enum TdispTdiState {
    /// The TDISP state is not initialized or indeterminate.
    Uninitialized,

    /// `TDI.Unlocked`` - The device is in its default "reset" state. Resources can be configured
    /// and no functionality can be used. Attestation cannot take place until the device has
    /// been locked.
    Unlocked,

    /// `TDI.Locked`` - The device resources have been locked and attestation can take place. The
    /// device's resources have been mapped and configured in hardware, but the device has not
    /// been attested. The platform will not allow the device to be functional until it has
    /// passed attestation and all device resources have been accepted into the guest context.
    Locked,

    /// `TDI.Run`` - The device is fully functional and attestation has succeeded. The device's
    /// resources have been mapped and accepted into the guest context. The device is ready to
    /// be used.
    Run,
}

impl From<TdispTdiState> for u64 {
    fn from(value: TdispTdiState) -> Self {
        match value {
            TdispTdiState::Uninitialized => 0,
            TdispTdiState::Unlocked => 1,
            TdispTdiState::Locked => 2,
            TdispTdiState::Run => 3,
        }
    }
}

impl From<u64> for TdispTdiState {
    fn from(value: u64) -> Self {
        match value {
            0 => TdispTdiState::Uninitialized,
            1 => TdispTdiState::Unlocked,
            2 => TdispTdiState::Locked,
            3 => TdispTdiState::Run,
            _ => TdispTdiState::Uninitialized,
        }
    }
}

/// The number of states to keep in the state history for debug.
const TDISP_STATE_HISTORY_LEN: usize = 10;

/// The reason for an `Unbind` call. `Unbind` can be called any time during the assignment flow.
#[derive(Debug)]
pub enum TdispUnbindReason {
    /// Unknown reason.
    Unknown(anyhow::Error),

    /// The device was unbound manually by the guest or host for a non-error reason.
    GuestInitiated(TdispGuestUnbindReason),

    /// The device attempted to perform an invalid state transition.
    ImpossibleStateTransition(anyhow::Error),

    /// The guest tried to transition the device to the Locked state while the device was not
    /// in the Unlocked state.
    InvalidGuestTransitionToLocked,

    /// The guest tried to retrieve the attestation report while the device was not in the
    /// Locked state.
    InvalidGuestGetAttestationReportState,

    /// The guest tried to accept the attestation report while the device was not in the
    /// Locked state.
    InvalidGuestAcceptAttestationReportState,

    /// The guest tried to unbind the device while the device with an unbind reason that is
    /// not recognized as a valid guest unbind reason. The unbind still succeeds but the
    /// recorded reason is discarded.
    InvalidGuestUnbindReason(anyhow::Error),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TdispGuestUnbindReason {
    /// The guest requested to unbind the device for an unspecified reason.
    Unknown,

    /// The guest requested to unbind the device because the device is being detached.
    Graceful,
}

impl From<TdispGuestUnbindReason> for u64 {
    fn from(value: TdispGuestUnbindReason) -> Self {
        match value {
            TdispGuestUnbindReason::Unknown => 0,
            TdispGuestUnbindReason::Graceful => 1,
        }
    }
}

impl From<u64> for TdispGuestUnbindReason {
    fn from(value: u64) -> Self {
        match value {
            1 => TdispGuestUnbindReason::Graceful,
            _ => TdispGuestUnbindReason::Unknown,
        }
    }
}

/// The state machine for the TDISP assignment flow for a device. Both the guest and host
/// synchronize this state machine with each other as they move through the assignment flow.
pub struct TdispHostStateMachine {
    /// The current state of the TDISP device emulator.
    current_state: TdispTdiState,
    /// A record of the last states the device was in.
    state_history: Vec<TdispTdiState>,
    /// The device ID of the device being assigned.
    debug_device_id: String,
    /// A record of the last unbind reasons for the device.
    unbind_reason_history: Vec<TdispUnbindReason>,
    /// Calls back into the host to perform TDISP actions.
    host_interface: Arc<Mutex<dyn TdispHostDeviceInterface>>,
}

impl TdispHostStateMachine {
    /// Create a new TDISP state machine with the `Unlocked` state.
    pub fn new(host_interface: Arc<Mutex<dyn TdispHostDeviceInterface>>) -> Self {
        Self {
            current_state: TdispTdiState::Unlocked,
            state_history: Vec::new(),
            debug_device_id: "".to_owned(),
            unbind_reason_history: Vec::new(),
            host_interface,
        }
    }

    /// Set the debug device ID string.
    pub fn set_debug_device_id(&mut self, debug_device_id: String) {
        self.debug_device_id = debug_device_id;
    }

    /// Print a debug message to the log.
    fn debug_print(&self, msg: &str) {
        tracing::error!(msg = format!("[TdispEmu] [{}] {}", self.debug_device_id, msg));
    }

    /// Print an error message to the log.
    fn error_print(&self, msg: &str) {
        tracing::error!(msg = format!("[TdispEmu] [{}] {}", self.debug_device_id, msg));
    }

    /// Get the current state of the TDI.
    fn state(&self) -> TdispTdiState {
        self.current_state
    }

    /// Check if the state machine can transition to the new state. This protects the underlying state machinery
    /// while higher level transition machinery tries to avoid these conditions. If the new state is impossible,
    /// `false` is returned.
    fn is_valid_state_transition(&self, new_state: &TdispTdiState) -> bool {
        match (self.current_state, new_state) {
            // Valid forward progress states from Unlocked -> Run
            (TdispTdiState::Unlocked, TdispTdiState::Locked) => true,
            (TdispTdiState::Locked, TdispTdiState::Run) => true,

            // Device can always return to the Unlocked state with `Unbind`
            (TdispTdiState::Run, TdispTdiState::Unlocked) => true,
            (TdispTdiState::Locked, TdispTdiState::Unlocked) => true,
            (TdispTdiState::Unlocked, TdispTdiState::Unlocked) => true,

            // Every other state transition is invalid
            _ => false,
        }
    }

    /// Check if the guest unbind reason is valid. This is used for bookkeeping purposes to
    /// ensure the guest unbind reason recorded in the unbind history is valid.
    fn is_valid_guest_unbind_reason(&self, reason: &TdispGuestUnbindReason) -> bool {
        !(matches!(reason, TdispGuestUnbindReason::Unknown))
    }

    /// Transitions the state machine to the new state if it is valid. If the new state is invalid,
    /// the state of the device is reset to the `Unlocked` state.
    fn transition_state_to(&mut self, new_state: TdispTdiState) -> anyhow::Result<()> {
        self.debug_print(&format!(
            "Request to transition from {:?} -> {:?}",
            self.current_state, new_state
        ));

        // Ensure the state transition is valid
        if !self.is_valid_state_transition(&new_state) {
            self.debug_print(&format!(
                "Invalid state transition {:?} -> {:?}",
                self.current_state, new_state
            ));
            return Err(anyhow::anyhow!(
                "Invalid state transition {:?} -> {:?}",
                self.current_state,
                new_state
            ));
        }

        // Record the state history
        if self.state_history.len() == TDISP_STATE_HISTORY_LEN {
            self.state_history.remove(0);
        }
        self.state_history.push(self.current_state);

        // Transition to the new state
        self.current_state = new_state;
        self.debug_print(&format!("Transitioned to {:?}", self.current_state));

        Ok(())
    }

    /// Transition the device to the `Unlocked` state regardless of the current state.
    fn unbind_all(&mut self, reason: TdispUnbindReason) -> anyhow::Result<()> {
        self.debug_print(&format!("Unbind called with reason {:?}", reason));

        // All states can be reset to the Unlocked state. This can only happen if the
        // state is corrupt beyond the state machine.
        if let Err(reason) = self.transition_state_to(TdispTdiState::Unlocked) {
            panic!(
                "[{}] Impossible state machine violation during TDISP Unbind: {:?}",
                self.debug_device_id, reason
            );
        }

        // Call back into the host to bind the device.
        let res = self
            .host_interface
            .lock()
            .tdisp_unbind()
            .context("host failed to unbind TDI");

        if let Err(e) = res {
            self.error_print(format!("Failed to unbind TDI: {:?}", e).as_str());
            return Err(e);
        }

        // Record the unbind reason
        if self.unbind_reason_history.len() == TDISP_STATE_HISTORY_LEN {
            self.unbind_reason_history.remove(0);
        }
        self.unbind_reason_history.push(reason);

        Ok(())
    }
}

/// Error returned by TDISP operations dispatched by the guest.
#[derive(Error, Debug, Copy, Clone)]
#[expect(missing_docs)]
pub enum TdispGuestOperationError {
    #[error("the operation was successful")]
    Success,
    #[error("the current TDI state is incorrect for this operation")]
    InvalidDeviceState,
    #[error("the reason for this unbind is invalid")]
    InvalidGuestUnbindReason,
    #[error("invalid TDI command ID")]
    InvalidGuestCommandId,
    #[error("operation requested was not implemented")]
    NotImplemented,
    #[error("host failed to process command")]
    HostFailedToProcessCommand,
}

impl From<TdispGuestOperationError> for u64 {
    fn from(err: TdispGuestOperationError) -> Self {
        match err {
            TdispGuestOperationError::Success => 0,
            TdispGuestOperationError::InvalidDeviceState => 1,
            TdispGuestOperationError::InvalidGuestUnbindReason => 2,
            TdispGuestOperationError::InvalidGuestCommandId => 3,
            TdispGuestOperationError::NotImplemented => 4,
            TdispGuestOperationError::HostFailedToProcessCommand => 5,
        }
    }
}

impl From<u64> for TdispGuestOperationError {
    fn from(err: u64) -> Self {
        match err {
            0 => TdispGuestOperationError::Success,
            1 => TdispGuestOperationError::InvalidDeviceState,
            2 => TdispGuestOperationError::InvalidGuestUnbindReason,
            3 => TdispGuestOperationError::InvalidGuestCommandId,
            4 => TdispGuestOperationError::NotImplemented,
            5 => TdispGuestOperationError::HostFailedToProcessCommand,
            _ => panic!("invalid TdispGuestOperationError code: {err}"),
        }
    }
}

/// Represents an interface by which guest commands can be dispatched to a
/// backing TDISP state handler. This could be an emulated TDISP device or an
/// assigned TDISP device that is actually connected to the guest.
pub trait TdispGuestRequestInterface {
    /// Transition the device from the Unlocked to Locked state. This takes place after the
    /// device has been assigned to the guest partition and the resources for the device have
    /// been configured by the guest. The device will be in the `Locked` state until it has
    /// been attested by the host.
    ///
    /// Attempting to transition the device to the `Locked` state while the device is not in the
    /// `Unlocked` state will unbind the device.
    fn request_lock_device_resources(&mut self) -> Result<(), TdispGuestOperationError>;

    /// Retrieves the attestation report for the device when the device is in the `Locked` state.
    /// The device will remain in the `Locked` state until the attestation report is validated by
    /// the guest and resources are accepted into the guest context.
    ///
    /// Attempting to retrieve the attestation report while the device is not in the `Locked` state
    /// will unbind the device.
    fn request_retrieve_attestation_report(&mut self) -> Result<(), TdispGuestOperationError>;

    /// Accepts the attestation report for the device when the device is in the `Locked` state.
    /// The device will now transition to the `Run` state. The device will not be functional in the
    /// guest until the resources are accepted into the guest context through the guest-to-firmware interface.
    ///
    /// Attempting to accept the attestation report while the device is not in the `Locked` state
    /// will unbind the device.
    fn request_accept_attestation_report(&mut self) -> Result<(), TdispGuestOperationError>;

    /// Guest initiates a graceful unbind of the device. The guest might
    /// initiate an unbind for a variety of reasons:
    ///  - Device is being detached/deactivated and is no longer needed in a functional state
    ///  - Device is powering down or entering a reset
    ///
    /// The device will transition to the `Unlocked` state. The guest can call
    /// this function at any time in any state to reset the device to the
    /// `Unlocked` state.
    fn request_unbind(
        &mut self,
        reason: TdispGuestUnbindReason,
    ) -> Result<(), TdispGuestOperationError>;
}

impl TdispGuestRequestInterface for TdispHostStateMachine {
    fn request_lock_device_resources(&mut self) -> Result<(), TdispGuestOperationError> {
        // If the guest attempts to transition the device to the Locked state while the device
        // is not in the Unlocked state, the device is reset to the Unlocked state.
        if self.current_state != TdispTdiState::Unlocked {
            self.error_print(
                "Unlocked to Locked state called while device was not in Unlocked state.",
            );

            self.unbind_all(TdispUnbindReason::InvalidGuestTransitionToLocked)
                .map_err(|_| TdispGuestOperationError::HostFailedToProcessCommand)?;
            return Err(TdispGuestOperationError::InvalidDeviceState);
        }

        self.debug_print(
            "Device bind requested, trying to transition from Unlocked to Locked state",
        );

        // Call back into the host to bind the device.
        let res = self
            .host_interface
            .lock()
            .tdisp_bind()
            .context("failed to call to bind TDI");

        if let Err(e) = res {
            self.error_print(format!("Failed to bind TDI: {:?}", e).as_str());
            return Err(TdispGuestOperationError::HostFailedToProcessCommand);
        }

        self.debug_print("Device transition from Unlocked to Locked state");
        self.transition_state_to(TdispTdiState::Locked).unwrap();
        Ok(())
    }

    fn request_retrieve_attestation_report(&mut self) -> Result<(), TdispGuestOperationError> {
        if self.current_state != TdispTdiState::Locked {
            self.error_print(
                "Retrieve attestation report called while device was not in Locked state.",
            );
            self.unbind_all(TdispUnbindReason::InvalidGuestGetAttestationReportState)
                .map_err(|_| TdispGuestOperationError::HostFailedToProcessCommand)?;

            return Err(TdispGuestOperationError::InvalidDeviceState);
        }

        // [TDISP TODO] Implement the attestation report retrieval.
        self.debug_print("Retrieve attestation report called successfully");
        Ok(())
    }

    fn request_accept_attestation_report(&mut self) -> Result<(), TdispGuestOperationError> {
        if self.current_state != TdispTdiState::Locked {
            self.error_print(
                "Accept attestation report called while device was not in Locked state.",
            );
            self.unbind_all(TdispUnbindReason::InvalidGuestAcceptAttestationReportState)
                .map_err(|_| TdispGuestOperationError::HostFailedToProcessCommand)?;

            return Err(TdispGuestOperationError::InvalidDeviceState);
        }

        // The guest accepts the attestation report and the device transitions to the Run state.
        self.debug_print(
            "Accept attestation report called successfully, device transitioning to Run state",
        );
        self.transition_state_to(TdispTdiState::Run).unwrap();
        Ok(())
    }

    fn request_unbind(
        &mut self,
        reason: TdispGuestUnbindReason,
    ) -> Result<(), TdispGuestOperationError> {
        // The guest can provide a reason for the unbind. If the unbind reason isn't valid for a guest (such as
        // if the guest says it is unbinding due to a host-related error), the reason is discarded and InvalidGuestUnbindReason
        // is recorded in the unbind history.
        let reason = if !self.is_valid_guest_unbind_reason(&reason) {
            let error_txt = format!("Invalid guest unbind reason {:?} requested", reason);

            self.error_print(error_txt.as_str());

            TdispUnbindReason::InvalidGuestUnbindReason(anyhow::anyhow!(error_txt))
        } else {
            TdispUnbindReason::GuestInitiated(reason)
        };

        self.debug_print(&format!(
            "Guest request to unbind succeeds while device is in {:?} (reason: {:?})",
            self.current_state, reason
        ));

        self.unbind_all(reason)
            .map_err(|_| TdispGuestOperationError::HostFailedToProcessCommand)?;

        Ok(())
    }
}
