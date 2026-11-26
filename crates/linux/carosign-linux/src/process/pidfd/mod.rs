//! Monitoring the process: `pidfd` and the death of an worker.
//!
//! ⚠️ Rule 09: The death of the process means that the hardware has calmed down. Observation
//! gives only one fact, "the process is alive or dead," and then the evidence of the driver
//! and the reset decide.
//!
//! ⚠️ `pidfd` preferably `kill(pid, 0)`The process number is reused, but the descriptor is
//! not.
//!
//! Real. `pidfd_open` live-in `libc`; here are the rules.

mod after_death;
mod process_state;
mod wait_budget;

pub use after_death::{AfterDeath, after_death};
pub use process_state::ProcessState;
pub use wait_budget::WaitBudget;
