//! Observer link snapshot policy with one-time tolerances.
//!
//! It's not a system policy owner, it's not a peer authority check. You can't order an
//! external record with a security clearance of one inotify: there's a race between the last
//! check and the sending, and you can only connect to production after the recording and the
//! security clearance are owned by the same trusted process.

#![allow(dead_code)]

mod owner_state;
mod policy_owner;
mod state;
mod watched_policy_admissions;

use owner_state::OwnerState;
#[cfg(test)]
pub(crate) use policy_owner::{PolicyOwner, serve_installer_update};
use state::State;
pub(crate) use watched_policy_admissions::WatchedPolicyAdmissions;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
