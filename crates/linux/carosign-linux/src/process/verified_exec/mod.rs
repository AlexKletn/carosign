//! Proven Launch: What Must Be True Before `exec`.
//!
//! The worker communicates only over the supplied channel. Create it before `exec` and close
//! the worker end in the parent afterward. See specification 09.
//!
//! ⚠️ Here are the rules, not the kernel calls: the real launch`execveat` s
//! `O_CLOEXEC`/`AT_EMPTY_PATH`) will come with a link to `libc`.

mod launch_error;
mod preconditions;

pub use launch_error::LaunchError;
pub use preconditions::{Preconditions, check};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
