//! Stages of a separate launch assistant.
//!
//! The success of cgroup and gate resolution does not yet confirm the other limitations, but
//! the full transition is only available when the assistant's descriptors and the caller's
//! verified composition are handed over.

#![allow(dead_code)]

mod enter;
mod enter_owned;
mod execute_prepared;
mod fail;
mod helper_failure;
mod helper_stage;
mod owned_execution;
mod owned_helper;
mod owned_runtime_file;
mod prepare_root;
mod runtime_file;
mod runtime_identity;

#[cfg(test)]
pub(crate) use enter::enter;
#[cfg(test)]
use enter::{after_gate, after_gate_with};
#[cfg(test)]
pub(super) use enter_owned::after_gate_owned;
pub(crate) use enter_owned::enter_owned;
#[cfg(test)]
use execute_prepared::execute_prepared;
use fail::{fail, terminate};
pub(crate) use helper_failure::HelperFailure;
pub(crate) use helper_stage::HelperStage;
pub(crate) use owned_execution::OwnedExecution;
#[cfg(test)]
pub(crate) use owned_helper::OwnedHelper;
pub(crate) use owned_runtime_file::OwnedRuntimeFile;
use prepare_root::prepare_root_with;
pub(crate) use runtime_file::RuntimeFile;
pub(crate) use runtime_identity::RuntimeIdentity;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
