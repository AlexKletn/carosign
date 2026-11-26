//! Installing a system call filter for a separate assistant.
//!
//! The list must come from a proven installation policy, and this module does not create such
//! a policy, nor does it confirm the other limits of isolation.

#![allow(dead_code)]

mod filter_error;
mod install;

pub(crate) use filter_error::FilterError;
pub(crate) use install::install;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
