//! Limited retention of cleaning results until handed over to the contractor.

use crate::operations::{CleanupFailure, CleanupProgress, OperationRegistry};
use crate::resources::TrustedCleanupBroker;
use carosign_core::ids::OperationId;
use carosign_engine::PlannedEffect;
use std::collections::BTreeMap;
use std::future::{Future, poll_fn};
use std::sync::Mutex;
use std::task::Poll;

mod limits;
pub use limits::CLEANUP_RESULTS_MAX;

#[path = "cleanup_owner.tests.rs"]
#[cfg(test)]
mod tests;

mod cleanup_result;
pub use cleanup_result::CleanupResult;
mod slot;
use slot::Slot;
mod cleanup_owner;
pub use cleanup_owner::CleanupOwner;
mod cleanup_owner_error;
pub use cleanup_owner_error::CleanupOwnerError;
mod cleanup_claim;
pub(crate) use cleanup_claim::CleanupClaim;
mod reservation;
use reservation::Reservation;
