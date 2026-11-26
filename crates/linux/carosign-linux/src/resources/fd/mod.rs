//! Descriptors: Ownership, rights and honest review.
//!
//! ⚠️ The rule for which this file exists (document 09): ** the descriptor given remains
//! available** even after our copy is closed.  ⁇  "Revoked leasing"  ⁇  "revoked access."
//! Only the completion of the process is taken away, and the security of the device is a
//! separate certificate.
//!
//! This module tests resource handling without kernel calls. Syscalls and descriptor
//! transfer belong to the process and transport layers.

mod access_mode;
mod delegation;
mod delegation_error;
mod handle_kind;
mod owned_fd;
mod revocation;
mod role;

pub use access_mode::AccessMode;
pub use delegation::{Delegation, may_delegate};
pub use delegation_error::DelegationError;
pub use handle_kind::HandleKind;
pub use owned_fd::OwnedFd;
pub use revocation::{Revocation, revocation_after_handoff};
pub use role::Role;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
