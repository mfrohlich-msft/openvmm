// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//!
//! WARNING: *** This crate is a work in progress, do not use in production! ***
//!
//! This module provides resources and traits for a TDISP client device
//! interface for OpenHCL devices.
//!
//! See: `vm/tdisp` for more information.
//! See: `openhcl_tdisp` for more information.

use inspect::Inspect;
use std::future::Future;
use std::sync::Arc;
use tdisp::GuestToHostCommand;
use tdisp::GuestToHostResponse;
pub use tdisp::TdispCommandId;
use tdisp::TdispGuestUnbindReason;
use tdisp::TdispUnbindReason;
pub use tdisp::{TDISP_INTERFACE_VERSION_MAJOR, TDISP_INTERFACE_VERSION_MINOR};

/// Represents a TDISP device assigned to a guest partition. This trait allows
/// the guest to send TDISP commands to the host through the backing interface.
/// [TDISP TODO] Change out `anyhow` for a `TdispError` type.
pub trait ClientDevice: Send + Sync + Inspect {
    /// Send a TDISP command to the host through the backing interface.
    fn tdisp_command_to_host(
        &self,
        command: GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse>;

    /// Checks if the device is TDISP capable and returns the device interface info if so.
    fn tdisp_get_device_interface_info(&self) -> anyhow::Result<tdisp::TdispDeviceInterfaceInfo>;

    /// Bind the device to the current partition and transition to Locked.
    fn tdisp_bind_interface(&self) -> anyhow::Result<()>;
}

/// Trait for registering TDISP devices.
pub trait RegisterTdisp: Send {
    /// Registers a TDISP capable device on the host.
    fn register(&mut self, target: Arc<dyn tdisp::TdispHostDeviceTarget>);
}

/// No operation struct for tests to implement `RegisterTdisp`.
pub struct TestTdispRegisterNoOp {}

impl RegisterTdisp for TestTdispRegisterNoOp {
    fn register(&mut self, _target: Arc<dyn tdisp::TdispHostDeviceTarget>) {
        todo!()
    }
}

pub trait VpciTdispInterface: Send + Sync {
    /// Sends a TDISP command to the device through the VPCI channel.
    fn send_tdisp_command(
        &self,
        payload: GuestToHostCommand,
    ) -> impl Future<Output = Result<GuestToHostResponse, anyhow::Error>> + Send;

    /// Get the TDISP interface info for the device.
    fn tdisp_get_device_interface_info(
        &self,
    ) -> impl Future<Output = anyhow::Result<tdisp::TdispDeviceInterfaceInfo>> + Send;

    /// Request the device to bind to the current partition and transition to Locked.
    fn tdisp_bind_interface(&self) -> impl Future<Output = anyhow::Result<()>> + Send;

    /// Request to unbind the device and return to the Unlocked state.
    fn tdisp_unbind(
        &self,
        reason: TdispGuestUnbindReason,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}
