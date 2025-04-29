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
pub struct TdispVfioClientDevice {
    /// Hypercall interface to the host.
    mshv_hvcall: hcl::ioctl::MshvHvcall,

    /// Hypervisor device ID.
    device_id: u64,
}

impl TdispVfioClientDevice {
    /// Creates a new `VfioClientDevice` instance.
    pub fn new(device_id: u64) -> anyhow::Result<Self> {
        let mshv_hvcall = hcl::ioctl::MshvHvcall::new().context("failed to open mshv_hvcall")?;
        mshv_hvcall.set_allowed_hypercalls(&[hvdef::HypercallCode::HvCallTdispDispatch]);

        Ok(Self {
            mshv_hvcall,
            device_id,
        })
    }
}

impl ClientDevice for TdispVfioClientDevice {
    fn tdisp_command_to_host(&self, mut command: GuestToHostCommand) -> anyhow::Result<()> {
        tracing::debug!("tdisp command to host: {}", command);
        command.device_id = self.device_id;
        self.mshv_hvcall
            .tdisp_dispatch(command)
            .context("failed to dispatch TDISP command")?;

        Ok(())
    }
}

impl Inspect for TdispVfioClientDevice {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond().field("tdisp-client", self);
    }
}
