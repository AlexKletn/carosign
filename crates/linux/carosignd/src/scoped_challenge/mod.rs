//! One-time accounting of calls already issued by a trusted source.
//!
//! This accounting itself does not establish trust in the source, recipient, or session.
//! It can only be connected to positive verification with the owner, who independently
//! confirms the origin of the recorded context.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;

use carosign_core::ids::OperationId;
use carosign_core::operation::context::BootInstant;
use carosign_evidence::recipient::ChallengeRef;
use carosign_runtime::ports::ScopedAssertionContext;

#[path = "scoped_challenge.tests.rs"]
#[cfg(test)]
mod tests;

mod scoped_challenges;
pub(crate) use scoped_challenges::ScopedChallenges;
mod entry;
use entry::Entry;
mod challenge_error;
pub(crate) use challenge_error::ChallengeError;
