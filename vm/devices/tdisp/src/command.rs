// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use crate::TdispGuestOperationError;
use crate::TdispTdiState;
use hvdef::hypercall::TdispGuestToHostResponse;
use std::fmt::Display;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

/// Represents a TDISP command sent from the guest to the host.
#[derive(Debug, Copy, Clone)]
pub struct GuestToHostCommand {
    /// The GPA of the response page.
    pub response_gpa: u64,
    /// Device ID of the target device.
    pub device_id: u64,
    /// The command ID.
    pub command_id: TdispCommandId,
}

impl From<hvdef::hypercall::TdispGuestToHostCommand> for GuestToHostCommand {
    fn from(value: hvdef::hypercall::TdispGuestToHostCommand) -> Self {
        Self {
            response_gpa: value.response_gpa,
            device_id: value.device_id,
            command_id: value.command_id.into(),
        }
    }
}

impl From<GuestToHostCommand> for hvdef::hypercall::TdispGuestToHostCommand {
    fn from(value: GuestToHostCommand) -> Self {
        Self {
            response_gpa: value.response_gpa,
            device_id: value.device_id,
            command_id: value.command_id.into(),
        }
    }
}

/// Represents a response from a TDISP command sent to the host by a guest.
#[derive(Debug, Copy, Clone)]
pub struct GuestToHostResponse {
    /// The command ID.
    pub command_id: TdispCommandId,
    /// The result status of the command.
    pub result: TdispGuestOperationError,
    /// The state of the TDI before the command was executed.
    pub tdi_state_before: TdispTdiState,
    /// The state of the TDI after the command was executed.
    pub tdi_state_after: TdispTdiState,
    /// The payload of the response if it has one.
    pub payload: TdispCommandResponsePayload,
}

impl From<TdispGuestToHostResponse> for GuestToHostResponse {
    fn from(value: TdispGuestToHostResponse) -> Self {
        Self {
            command_id: value.command_id.into(),
            result: value.result.into(),
            tdi_state_before: tdisp_state_from_hvcall(value.tdi_state_before),
            tdi_state_after: tdisp_state_from_hvcall(value.tdi_state_after),
            // [TDISP TODO] This is a placeholder for a better serialization mechanism.
            payload: deserialize_payload(&value).unwrap(),
        }
    }
}

impl From<GuestToHostResponse> for TdispGuestToHostResponse {
    fn from(value: GuestToHostResponse) -> Self {
        let mut obj = Self {
            command_id: value.command_id.into(),
            result: value.result.into(),
            tdi_state_before: tdisp_state_to_hvcall(value.tdi_state_before),
            tdi_state_after: tdisp_state_to_hvcall(value.tdi_state_after),
            payload: [0; 2048],
        };

        // [TDISP TODO] This is a placeholder for a better serialization mechanism.
        serialize_payload(&value.payload, &mut obj.payload).unwrap();

        obj
    }
}

/// [TDISP TODO] This is a placeholder for a better serialization mechanism.
fn deserialize_payload(
    command: &TdispGuestToHostResponse,
) -> anyhow::Result<TdispCommandResponsePayload> {
    match command.command_id.into() {
        TdispCommandId::GetDeviceInterfaceInfo => {
            let payload = TdispDeviceInterfaceInfo::read_from_bytes(
                &command.payload[0..size_of::<TdispDeviceInterfaceInfo>()],
            )
            .map_err(|_| anyhow::anyhow!("failed to deserialize GetDeviceInterfaceInfo payload"))?;
            Ok(TdispCommandResponsePayload::GetDeviceInterfaceInfo(payload))
        }
        TdispCommandId::Bind => Ok(TdispCommandResponsePayload::None),
        _ => Ok(TdispCommandResponsePayload::None),
    }
}

/// [TDISP TODO] This is a placeholder for a better serialization mechanism.
fn serialize_payload(
    payload: &TdispCommandResponsePayload,
    target: &mut [u8],
) -> anyhow::Result<()> {
    match payload {
        TdispCommandResponsePayload::GetDeviceInterfaceInfo(payload) => payload
            .write_to(&mut target[0..size_of::<TdispDeviceInterfaceInfo>()])
            .map_err(|e| {
                anyhow::anyhow!("failed to serialize GetDeviceInterfaceInfo payload: {}", e)
            }),
        TdispCommandResponsePayload::None => Ok(()),
    }
}

impl Display for GuestToHostCommand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Display the Debug representation of the command.
        f.debug_struct("GuestToHostCommand")
            .field("command_id", &self.command_id)
            .finish()
    }
}

fn tdisp_state_from_hvcall(tdi_state: u64) -> TdispTdiState {
    match tdi_state {
        0 => TdispTdiState::Uninitialized,
        1 => TdispTdiState::Unlocked,
        2 => TdispTdiState::Locked,
        3 => TdispTdiState::Run,
        4 => TdispTdiState::Error,
        _ => TdispTdiState::Uninitialized,
    }
}

fn tdisp_state_to_hvcall(tdi_state: TdispTdiState) -> u64 {
    match tdi_state {
        TdispTdiState::Uninitialized => 0,
        TdispTdiState::Unlocked => 1,
        TdispTdiState::Locked => 2,
        TdispTdiState::Run => 3,
        TdispTdiState::Error => 4,
    }
}

/// Represents a TDISP command sent from the guest to the host.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TdispCommandId {
    /// Invalid command id.
    Unknown,

    /// Request the device's TDISP interface information.
    GetDeviceInterfaceInfo,

    /// Bind the device to the current partition and transition to Locked.
    Bind,

    /// Get the TDI report for attestation from the host for the device.
    GetTdiReport,

    /// Transition the device to the Start state after successful attestation.
    StartTdi,

    /// Unbind the device from the partition, reverting it back to the Unlocked state.
    Unbind,
}

impl From<TdispCommandId> for u64 {
    fn from(value: TdispCommandId) -> Self {
        match value {
            TdispCommandId::Unknown => 0,
            TdispCommandId::GetDeviceInterfaceInfo => 1,
            TdispCommandId::Bind => 2,
            TdispCommandId::GetTdiReport => 3,
            TdispCommandId::StartTdi => 4,
            TdispCommandId::Unbind => 5,
        }
    }
}

impl From<u64> for TdispCommandId {
    fn from(value: u64) -> Self {
        match value {
            0 => TdispCommandId::Unknown,
            1 => TdispCommandId::GetDeviceInterfaceInfo,
            2 => TdispCommandId::Bind,
            3 => TdispCommandId::GetTdiReport,
            4 => TdispCommandId::StartTdi,
            5 => TdispCommandId::Unbind,
            _ => TdispCommandId::Unknown,
        }
    }
}

/// Represents the TDISP device interface information, such as the version and supported features.
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct TdispDeviceInterfaceInfo {
    pub interface_version_major: u32,
    pub interface_version_minor: u32,

    /// [TDISP TODO] Placeholder for bitfield advertising feature set capabilities.
    pub supported_features: u64,
}

#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct TdispGuestInterfaceInfo {
    /// The major version for the interface. This does not necessarily match to a TDISP specification version.
    /// [TDISP TODO] dead_code
    #[expect(dead_code)]
    pub interface_version_major: u32,

    /// The minor version for the interface. This does not necessarily match to a TDISP specification version.
    /// [TDISP TODO] dead_code
    #[expect(dead_code)]
    pub interface_version_minor: u32,
}

/// Serialized to and from the payload field of a TdispCommandResponse
#[derive(Debug, Copy, Clone)]
pub enum TdispCommandResponsePayload {
    None,

    /// TdispCommandId::GetDeviceInterfaceInfo
    GetDeviceInterfaceInfo(TdispDeviceInterfaceInfo),
}

impl From<TdispDeviceInterfaceInfo> for TdispCommandResponsePayload {
    fn from(value: TdispDeviceInterfaceInfo) -> Self {
        TdispCommandResponsePayload::GetDeviceInterfaceInfo(value)
    }
}
