//! Cleaning the descriptors of a single-threaded assistant before exec.
//!
//! The FD table can be shared with other threads, and this stage does not confirm the origin
//! or isolation of the worker.

#![allow(dead_code)]

mod allowed_fd;
mod exec_disposition;
mod retain_only;

pub(crate) use allowed_fd::AllowedFd;
pub(crate) use exec_disposition::ExecDisposition;
pub(crate) use retain_only::retain_only;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
