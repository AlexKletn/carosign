//! Processes of workers: verified launch, sandbox, observation.
//!
//! ⚠️ The order is mandatory: condition checks → channel creation → channel launches →
//! channel transfers. Each step is a separate step, so that the failure calls its place.

pub(crate) mod cgroup;
pub(crate) mod fd_cleanup;
pub(crate) mod gate;
pub(crate) mod helper;
pub(crate) mod namespaces;
pub mod pidfd;
pub(crate) mod pinned_exec;
pub(crate) mod policy_admissions;
pub(crate) mod privileges;
pub(crate) mod root_switch;
pub(crate) mod runtime_mount;
pub mod sandbox;
pub(crate) mod seccomp;
pub mod spawn;
pub(crate) mod tmpfs_root;
pub mod verified_exec;
