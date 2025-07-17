// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//!
//! WARNING: *** This crate is a work in progress, do not use in production! ***
//!
//! This module provides an implementation of the TDISP client device
//! interface for OpenHCL devices.
//!
//! See: `vm/tdisp` for more information.

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(missing_docs)]

use std::future::Future;

use inspect::Inspect;
use openhcl_tdisp_resources::ClientDevice;
use tdisp::GuestToHostCommand;
use tdisp::GuestToHostResponse;
use tdisp::TdispCommandId;
use tdisp::TdispCommandResponsePayload;
use tdisp::TdispTdiState;

/// Implements the `ClientDevice` trait for a VFIO device.
pub struct TdispOpenHclClientDevice {}
impl TdispOpenHclClientDevice {
    pub fn new() -> Self {
        Self {}
    }

    pub fn send_command_to_host(
        &self,
        command: &mut GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse> {
        todo!()
    }

    pub fn read_response(
        &self,
        command: &GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse> {
        todo!()
    }
}

impl ClientDevice for TdispOpenHclClientDevice {
    fn tdisp_command_to_host(
        &self,
        mut command: GuestToHostCommand,
    ) -> anyhow::Result<GuestToHostResponse> {
        tracing::info!("tdisp_command_to_host: command = {:?}", &command);

        self.send_command_to_host(&mut command)?;

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

        // [TDISP TODO] Ensure valid state transitions, take defensive approach to error handling.

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

impl Inspect for TdispOpenHclClientDevice {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond().field("tdisp-client", self);
    }
}
