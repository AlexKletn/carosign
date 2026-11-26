//! Changes: Registration and Deletion Requests and Their Outcomes
//!
//! ⚠️ [`MutationOutcome::NeedsReconciliation`] - **honest state: the hardware could say "write it
//! down," and it's not known if it passed. `NoChange`, not read as an error - until an
//! independent verification proves otherwise, the area remains locked (document 10).
//!
//! Removal and replacement are fixed by the generation: `expected_generation` checks
//! **to**sending and **to**registering again (Scenario T5).

mod enroll_request;
mod mutation_failure;
mod mutation_outcome;
mod remove_request;

pub use enroll_request::EnrollRequest;
pub use mutation_failure::MutationFailure;
pub use mutation_outcome::MutationOutcome;
pub use remove_request::RemoveRequest;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
