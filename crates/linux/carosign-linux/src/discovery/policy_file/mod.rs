//! Limited reading of the policy file regarding the fixed directory.
//!
//! The content remains unconfirmed bytes. Checking the DAC and reading from the retained
//! descriptors does not establish the origin, immutability, or authority of the policy: it is
//! the task of the installation system adapter.

#![allow(dead_code)]

mod policy_read_error;
mod read;

pub(super) use policy_read_error::PolicyReadError;
#[cfg(test)]
pub(super) use read::read;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
