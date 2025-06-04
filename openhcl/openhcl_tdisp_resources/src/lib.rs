// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! This module provides resources and traits for a TDISP client device
//! interface for OpenHCL VFIO devices.
//!
//! See: `vm/tdisp` for more information.
//! See: `openhcl_tdisp` for more information.

use inspect::Inspect;
use std::sync::Arc;
use tdisp::GuestToHostCommand;
use tdisp::GuestToHostResponse;
pub use tdisp::TdispCommandId;

/// Represents a TDISP device assigned to a guest partition. This trait allows
/// the guest to send TDISP commands to the host through the backing hypercall
/// interface.
pub trait ClientDevice: Send + Sync + Inspect {
    /// Send a TDISP command to the host through backing hypercall interface.
    fn tdisp_command_to_host(
        &self,
        command: GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse>;

    /// Send a TDISP command to the host through backing hypercall interface with no arguments.
    fn tdisp_command_no_args(
        &self,
        command_id: TdispCommandId,
    ) -> anyhow::Result<GuestToHostResponse>;
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
