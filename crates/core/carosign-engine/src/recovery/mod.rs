//! Restoration by magazine: where the cliff is dangerous.
//!
//! ⚠️ The parsing of document 10 ("Crash windows") is a **list of windows, not actions. It
//! describes what the log does; the actions themselves are in
//! time.`carosign-runtime::recovery`).
//!
//! The main rule: record `DispatchArmed` On the disk, it means that the hardware **could change
//! even if the break happened before it was sent, so the recovery never sends it back.

mod may_replay_mutating;
mod recovery_action;

pub use may_replay_mutating::may_replay_mutating;
pub use recovery_action::{RecoveryAction, action_for};

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
