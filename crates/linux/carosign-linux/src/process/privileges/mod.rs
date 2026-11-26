//! Verified limitations of individual assistant privileges.
//!
//! These steps are irreversible and must only be performed by a single-threaded process and
//! do not confirm the namespace, seccomp, or origin of the packet.

#![allow(dead_code)]

mod capability_data;
mod capability_header;
mod drop_identity;
mod privilege_error;
mod privilege_stage;
mod set_no_new_privileges;

use capability_data::CapabilityData;
use capability_header::CapabilityHeader;
pub(crate) use drop_identity::drop_identity;
pub(crate) use privilege_error::PrivilegeError;
pub(crate) use privilege_stage::PrivilegeStage;
pub(crate) use set_no_new_privileges::set_no_new_privileges;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
