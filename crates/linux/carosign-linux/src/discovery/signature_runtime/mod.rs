//! Preparation of fixed files of the dynamic environment of the checker.
//!
//! The list and measurements themselves must come from an independent system policy, and this
//! training does not confirm the completeness of the ELF dependencies, the origin of the
//! policy, the isolation of the root, or the success of the gpgv.
#![allow(dead_code)]

mod entry;
mod pinned_runtime;
mod runtime_error;
mod runtime_file;

use entry::Entry;
pub(super) use pinned_runtime::PinnedRuntime;
#[cfg(test)]
use pinned_runtime::valid_name;
pub(super) use runtime_error::RuntimeError;
pub(super) use runtime_file::RuntimeFile;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
