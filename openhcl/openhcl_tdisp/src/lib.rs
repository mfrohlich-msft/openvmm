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

    /// Reads the response from the hypercall after it executed successfully.
    fn read_response(&self, command: &GuestToHostCommand) -> anyhow::Result<GuestToHostResponse> {
        let response: TdispGuestToHostResponse = self.response_buffer.read_obj(0);
        let response_id: TdispCommandId = command.command_id.into();
        if response_id != command.command_id {
            return Err(anyhow::anyhow!(
                "response command ID mismatch, expected {:?}, got {:?}",
                command.command_id,
                response.command_id
            ));
        }

        Ok(response.into())
    }
}

impl ClientDevice for TdispVfioClientDevice {
    fn tdisp_command_to_host(&self, mut command: GuestToHostCommand) -> anyhow::Result<()> {
        tracing::debug!("tdisp command to host: {}", command);
        command.device_id = self.device_id;
        self.mshv_hvcall
            .tdisp_dispatch(command)
            .context("failed to dispatch TDISP command")?;

        // Response has now been written to the response buffer.
        let resp = self.read_response(&command)?;

        tracing::error!("tdisp_command_to_host: response = {:?}", &resp);

        Ok(resp)
    }

    fn tdisp_command_no_args(
        &self,
        command_id: TdispCommandId,
    ) -> anyhow::Result<GuestToHostResponse> {
        self.tdisp_command_to_host(GuestToHostCommand {
            // Filled in later.
            response_gpa: 0,
            device_id: 0,
            command_id,
        })
    }
}

impl Inspect for TdispVfioClientDevice {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond().field("tdisp-client", self);
    }
}
