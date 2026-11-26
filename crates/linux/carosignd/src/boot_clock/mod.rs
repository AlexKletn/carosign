//! System service hours tied to the current load.

use std::sync::Mutex;

use carosign_core::ids::BootId;
use carosign_core::operation::context::BootInstant;
use carosign_linux::sys::{self, SysError};
use carosign_runtime::ports::TimeSource;

#[path = "boot_clock.tests.rs"]
#[cfg(test)]
mod tests;

mod boot_clock;
pub(crate) use boot_clock::BootClock;
mod clock_state;
use clock_state::ClockState;
