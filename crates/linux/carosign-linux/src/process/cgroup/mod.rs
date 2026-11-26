//! Prepare a limited cgroup v2 in a pre-delegated directory.
//!
//! This module does not confirm delegation, origin of the process, or isolation. The
//! assistant can only attach itself and then wait for the gate; this transition alone does
//! not confirm other start conditions.

#![allow(dead_code)]

mod cgroup_error;
mod limits;
mod prepared_cgroup;

pub(crate) use cgroup_error::CgroupError;
pub(crate) use limits::Limits;
pub(crate) use prepared_cgroup::PreparedCgroup;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
