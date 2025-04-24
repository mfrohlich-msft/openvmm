// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! TDISP is a standardized interface for end-to-end encryption and attestation
//! of trusted assigned devices to confidential/isolated partitions. This crate
//! implements structures and interfaces for the host and guest to prepare and
//! assign trusted devices. Examples of technologies that implement TDISP
//! include:
//! - Intel® "TDX Connect"
//! - AMD SEV-TIO

use inspect::Inspect;
use std::fmt::Display;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

/// Represents a TDISP command sent from the guest to the host.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct GuestToHostCommand {
    /// The command ID.
    pub command_id: u64,
}

impl From<hvdef::hypercall::TdispGuestToHostCommand> for GuestToHostCommand {
    fn from(value: hvdef::hypercall::TdispGuestToHostCommand) -> Self {
        Self {
            command_id: value.command_id,
        }
    }
}

impl From<GuestToHostCommand> for hvdef::hypercall::TdispGuestToHostCommand {
    fn from(value: GuestToHostCommand) -> Self {
        Self {
            command_id: value.command_id,
        }
    }
}

/// Represents a TDISP device assigned to a guest partition. This trait allows
/// the guest to send TDISP commands to the host through the backing hypercall
/// interface.
pub trait ClientDevice: Send + Sync + Inspect {
    /// Send a TDISP command to the host through backing hypercall interface.
    fn tdisp_command_to_host(&self, command: GuestToHostCommand) -> anyhow::Result<()>;
}

impl Display for GuestToHostCommand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Display the Debug representation of the command.
        f.debug_struct("GuestToHostCommand")
            .field("command_id", &self.command_id)
            .finish()
    }
}
