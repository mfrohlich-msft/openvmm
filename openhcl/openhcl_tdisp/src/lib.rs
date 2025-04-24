// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! This module provides an implementation of the TDISP client device
//! interface for OpenHCL VFIO devices.
//!
//! See: `vm/tdisp` for more information.

use anyhow::Context;
use inspect::Inspect;
use tdisp::ClientDevice;
use tdisp::GuestToHostCommand;

/// Implements the `ClientDevice` trait for a VFIO device.
pub struct VfioClientDevice {
    /// Hypercall interface to the host.
    mshv_hvcall: hcl::ioctl::MshvHvcall,
}

impl VfioClientDevice {
    /// Creates a new `VfioClientDevice` instance.
    pub fn new() -> anyhow::Result<Self> {
        let mshv_hvcall = hcl::ioctl::MshvHvcall::new().context("failed to open mshv_hvcall")?;
        mshv_hvcall.set_allowed_hypercalls(&[hvdef::HypercallCode::HvCallTdispDispatch]);

        Ok(Self { mshv_hvcall })
    }
}

impl ClientDevice for VfioClientDevice {
    fn tdisp_command_to_host(&self, command: GuestToHostCommand) -> anyhow::Result<()> {
        tracing::debug!("tdisp command to host: {}", command);
        self.mshv_hvcall
            .tdisp_dispatch(command)
            .context("failed to dispatch TDISP command")?;

        Ok(())
    }
}

impl Inspect for VfioClientDevice {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond().field("tdisp-client", self);
    }
}
