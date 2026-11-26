//! Transition of a single-threaded assistant to a prepared tmpfs.
//!
//! This step does not confirm the composition of the runtime, the origin of the files, or the
//! entire sandbox, and if any failure occurs after the start, the caller must terminate.
#![allow(dead_code)]

mod enter;
mod switch_error;
mod switch_stage;

#[cfg(test)]
pub(crate) use enter::enter;
pub(crate) use enter::enter_prepared;
pub(crate) use switch_error::SwitchError;
pub(crate) use switch_stage::SwitchStage;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
