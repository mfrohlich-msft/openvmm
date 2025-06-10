// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! This module provides an implementation of the TDISP client device
//! interface for OpenHCL VFIO devices.
//!
//! See: `vm/tdisp` for more information.

use anyhow::Context;
use hvdef::hypercall::TdispGuestToHostResponse;
use inspect::Inspect;
use memoryblock::MemoryBlock;
use memoryblock::PAGE_SIZE;
use openhcl_tdisp_resources::ClientDevice;
use tdisp::GuestToHostCommand;
use tdisp::GuestToHostResponse;
use tdisp::TdispCommandId;
use tdisp::TdispCommandResponsePayload;
use tdisp::TdispTdiState;

/// Implements the `ClientDevice` trait for a VFIO device.
pub struct TdispVfioClientDevice {
    /// Hypercall interface to the host.
    mshv_hvcall: hcl::ioctl::MshvHvcall,

    /// Hypervisor device ID.
    device_id: u64,

    /// A page of memory to receive responses from the guest.
    /// [TDISP TODO] This should probably be a `MemoryBlock`.
    response_buffer: MemoryBlock,
}

const REQUIRED_RESPONSE_BUFFER_SIZE: usize = PAGE_SIZE;

impl TdispVfioClientDevice {
    /// Creates a new `VfioClientDevice` instance.
    pub fn new(device_id: u64, response_buffer: MemoryBlock) -> anyhow::Result<Self> {
        let mshv_hvcall = hcl::ioctl::MshvHvcall::new().context("failed to open mshv_hvcall")?;
        mshv_hvcall.set_allowed_hypercalls(&[hvdef::HypercallCode::HvCallTdispDispatch]);

        if response_buffer.len() < REQUIRED_RESPONSE_BUFFER_SIZE {
            return Err(anyhow::anyhow!(
                "response buffer is too small, expected at least {} bytes, got {}",
                REQUIRED_RESPONSE_BUFFER_SIZE,
                response_buffer.len()
            ));
        }

        // Ensure the response buffer is zeroed.
        response_buffer.write_at(0, &[0; REQUIRED_RESPONSE_BUFFER_SIZE]);

        Ok(Self {
            mshv_hvcall,
            device_id,
            response_buffer,
        })
    }

    /// Reads the response from the hypercall after it executed successfully.
    fn read_response(&self, command: &GuestToHostCommand) -> anyhow::Result<GuestToHostResponse> {
        let response: TdispGuestToHostResponse = self.response_buffer.read_obj(0);
        let response_id: TdispCommandId = command.command_id;
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
    fn tdisp_command_to_host(
        &self,
        mut command: GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse> {
        tracing::info!("tdisp_command_to_host: command = {:?}", &command);

        command.response_gpa = self.response_buffer.pfns()[0] * (PAGE_SIZE as u64);
        command.device_id = self.device_id;

        self.mshv_hvcall
            .tdisp_dispatch(command)
            .context("failed to dispatch TDISP command")?;

        // Response has now been written to the response buffer.
        let resp = self.read_response(&command)?;

        tracing::info!("tdisp_command_to_host: response = {:?}", &resp);
        if resp.tdi_state_after != resp.tdi_state_before {
            tracing::info!(
                "tdisp_command_to_host: TDI state transition performed, {:?} -> {:?}",
                resp.tdi_state_before,
                resp.tdi_state_after
            );
        } else {
            tracing::info!("tdisp_command_to_host: No TDI state transition.");
        }

        if resp.tdi_state_after == TdispTdiState::Error {
            tracing::error!("tdisp_command_to_host: TDI state transitioned to Error.");
            return Err(anyhow::anyhow!("TDI state transitioned to Error."));
        }

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

    /// Get the device interface info.
    fn tdisp_get_device_interface_info(&self) -> anyhow::Result<tdisp::TdispDeviceInterfaceInfo> {
        let res = self.tdisp_command_no_args(TdispCommandId::GetDeviceInterfaceInfo);

        match res {
            Ok(resp) => match resp.payload {
                TdispCommandResponsePayload::GetDeviceInterfaceInfo(info) => Ok(info),
                _ => Err(anyhow::anyhow!("unexpected response payload")),
            },
            Err(e) => Err(e),
        }
    }

    /// Bind the device to the current partition and transition to Locked.
    fn tdisp_bind_interface(&self) -> anyhow::Result<()> {
        let res = self.tdisp_command_no_args(TdispCommandId::Bind);
        match res {
            Ok(resp) => match resp.payload {
                TdispCommandResponsePayload::None => Ok(()),
                _ => Err(anyhow::anyhow!("unexpected response payload")),
            },
            Err(e) => Err(e),
        }
    }
}

impl Inspect for TdispVfioClientDevice {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond().field("tdisp-client", self);
    }
}
