//! Starting an Worker: Order and What Cannot Be Done After
//!
//! The worker communicates only over the supplied channel. Create it before `exec` and close
//! the worker end in the parent afterward. See specification 09.
//!
//! Steps describe externally received confirmations, rather than running.
//! The jump between them is a rejection.

mod launch;
mod launch_stage;
mod spawn_error;

pub use launch::Launch;
pub use launch_stage::LaunchStage;
pub use spawn_error::SpawnError;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
