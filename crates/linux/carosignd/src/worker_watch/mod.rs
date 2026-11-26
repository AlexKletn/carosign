//! Observe the process and protect its retained resources.
//!
//! The identity of the pidfd is determined by the owner of the launch; this transition does
//! not verify the origin of the match. The death of the process does not mean the device is
//! stopped or the permission to remove the fence.

use std::sync::Mutex;

use carosign_core::{
    device::DeviceEpoch,
    ids::{LeaseId, WorkerInstanceId},
};
use carosign_linux::sys::{PidFd, SysError};
use carosign_runtime::scheduler::{GrantResult, Request, Scheduler};

#[path = "worker_watch.tests.rs"]
#[cfg(test)]
mod tests;

mod watch_stage;
pub(crate) use watch_stage::WatchStage;
mod worker_watch;
#[cfg(test)]
pub(crate) use worker_watch::WorkerWatch;
