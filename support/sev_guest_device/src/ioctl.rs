// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! The module implements Linux SEV-SNP Guest APIs based on ioctl.

// UNSAFETY: unsafe needed to make ioctl calls.
#![expect(unsafe_code)]

use crate::protocol;
use std::fs::File;
use std::os::fd::AsRawFd;
use thiserror::Error;
use zerocopy::FromZeros;
use zerocopy::IntoBytes;

#[expect(missing_docs)] // self-explanatory fields
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to open /dev/sev-guest")]
    OpenDevSevGuest(#[source] std::io::Error),
    #[error("SNP_GET_REPORT ioctl failed")]
    SnpGetReportIoctl(#[source] nix::Error),
    #[error("SNP_GET_DERIVED_KEY ioctl failed")]
    SnpGetDerivedKeyIoctl(#[source] nix::Error),
    #[error("TIO_GUEST_REQUEST ioctl failed")]
    TioGuestRequestIoctl(#[source] nix::Error),
}

nix::ioctl_readwrite!(
    /// `SNP_GET_REPORT` ioctl defined by Linux.
    snp_get_report,
    protocol::SNP_GUEST_REQ_IOC_TYPE,
    0x0,
    // [TDISP TODO] Change this back since this is hacked to be a different struct right now.
    protocol::TioGuestRequestIoctl
);

nix::ioctl_readwrite!(
    /// `SNP_GET_DERIVED_KEY` ioctl defined by Linux.
    snp_get_derived_key,
    protocol::SNP_GUEST_REQ_IOC_TYPE,
    0x1,
// [TDISP TODO] Change this back since this is hacked to be a different struct right now.
    protocol::TioGuestRequestIoctl
);

nix::ioctl_readwrite!(
    /// `TIO_GUEST_REQUEST` ioctl defined by Linux.
    tio_guest_request,
    protocol::SNP_GUEST_REQ_IOC_TYPE,
    0x3,
    // [TDISP TODO] Use proper interface for this.
    protocol::TioGuestRequestIoctl
);

/// Abstraction of the /dev/sev-guest device.
pub struct SevGuestDevice {
    file: File,
}

impl SevGuestDevice {
    /// Open an /dev/sev-guest device
    pub fn open() -> Result<Self, Error> {
        let sev_guest = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/sev-guest")
            .map_err(Error::OpenDevSevGuest)?;

        Ok(Self { file: sev_guest })
    }

    /// Invoke the `SNP_GET_REPORT` ioctl via the device.
    pub fn get_report(&self, user_data: [u8; 64], vmpl: u32) -> Result<protocol::SnpReport, Error> {
        let req = protocol::SnpReportReq {
            user_data,
            vmpl,
            rsvd: [0u8; 28],
        };

        let resp = protocol::SnpReportResp::new_zeroed();

        let mut snp_guest_request = protocol::TioGuestRequestIoctl {
            msg_version: protocol::SNP_GUEST_REQ_MSG_VERSION,
            req_data: req.as_bytes().as_ptr() as u64,
            resp_data: resp.as_bytes().as_ptr() as u64,
            exitinfo: protocol::VmmErrorCode::new_zeroed(),
            exitinfo1: 0,
            msg_type: 0,
            req_size: 0,
            resp_size: 0,
            pci_id: 0,
            additional_arg: 0,
        };

        // SAFETY: Make SNP_GET_REPORT ioctl call to the device with correct types.
        unsafe {
            snp_get_report(self.file.as_raw_fd(), &mut snp_guest_request)
                .map_err(Error::SnpGetReportIoctl)?;
        }

        Ok(resp.report)
    }

    /// Invoke the `SNP_GET_DERIVED_KEY` ioctl via the device.
    pub fn get_derived_key(
        &self,
        root_key_select: u32,
        guest_field_select: u64,
        vmpl: u32,
        guest_svn: u32,
        tcb_version: u64,
    ) -> Result<[u8; protocol::SNP_DERIVED_KEY_SIZE], Error> {
        let req = protocol::SnpDerivedKeyReq {
            root_key_select,
            rsvd: 0u32,
            guest_field_select,
            vmpl,
            guest_svn,
            tcb_version,
        };

        let resp = protocol::SnpDerivedKeyResp::new_zeroed();

        let mut snp_guest_request = protocol::TioGuestRequestIoctl {
            msg_version: protocol::SNP_GUEST_REQ_MSG_VERSION,
            req_data: req.as_bytes().as_ptr() as u64,
            resp_data: resp.as_bytes().as_ptr() as u64,
            exitinfo: protocol::VmmErrorCode::new_zeroed(),
            exitinfo1: 0,
            msg_type: 0,
            req_size: 0,
            resp_size: 0,
            pci_id: 0,
            additional_arg: 0,
        };

        // SAFETY: Make SNP_GET_DERIVED_KEY ioctl call to the device with correct types
        unsafe {
            snp_get_derived_key(self.file.as_raw_fd(), &mut snp_guest_request)
                .map_err(Error::SnpGetReportIoctl)?;
        }

        Ok(resp.derived_key)
    }

    /// Invoke the `TIO_GUEST_REQUEST` ioctl via the device.
    pub fn tio_guest_request(&mut self) -> Result<(), Error> {
        let req = protocol::TioMsgTdiInfoReq {
            guest_device_id: 0,
            _reserved0: [0; 14],
        };

        let resp = protocol::TioMsgTdiInfoRsp::new_zeroed();

        let msg_type = 19; // TIO_MSG_TDI_INFO_REQ
        tracing::info!(
            msg = "Issuing TIO_GUEST_REQUEST ioctl with value guest device 0x1 (msg_type = {:?})",
            msg_type
        );
        let mut snp_guest_request = protocol::TioGuestRequestIoctl {
            msg_version: protocol::SNP_GUEST_REQ_MSG_VERSION,
            req_data: req.as_bytes().as_ptr() as u64,
            resp_data: resp.as_bytes().as_ptr() as u64,
            exitinfo: protocol::VmmErrorCode::new_zeroed(),
            exitinfo1: 0,
            msg_type, // TIO_MSG_TDI_INFO_REQ
            req_size: req.as_bytes().len() as u64,
            resp_size: resp.as_bytes().len() as u64,
            pci_id: 1, // TODO: Get the actual guest ID from the host
            additional_arg: 0,
        };

        // SAFETY: Make TIO_GUEST_REQUEST ioctl call to the device with correct types
        unsafe {
            tio_guest_request(self.file.as_raw_fd(), &mut snp_guest_request)
                .map_err(Error::TioGuestRequestIoctl)?;
        }

        tracing::info!(
            msg = "TIO_GUEST_REQUEST ioctl completed, guest_device_id = {:?}, tdi_status = {:?}",
            resp.guest_device_id,
            resp.tdi_status
        );

        Ok(())
    }
}
