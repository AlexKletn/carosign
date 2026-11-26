//! Create a detached empty tmpfs for the future root of the assistant.
//!
//! The resulting descriptor does not change the root of the process and does not prove
//! isolation: approved runtime files, root transition and closure of old descriptors are
//! performed by other steps.

#![allow(dead_code)]

mod assign_owner;
mod create;
mod root_error;
mod root_stage;

pub(crate) use assign_owner::assign_owner;
pub(crate) use create::create;
use create::error;
pub(crate) use root_error::RootError;
pub(crate) use root_stage::RootStage;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
