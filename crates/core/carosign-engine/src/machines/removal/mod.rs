//! Removal: Same table as the change, but with a fixed generation.
//!
//! ⚠️ Removal with an outdated generation is rejected by the **do**s of sending (T5
//! scenario): this is the
//! [[T]`MutationFailure::GenerationConflict`](carosign_core::operation::MutationFailure)It's
//! not an hardware outcome.
//!
//! ⚠️ Successful deletion of **not* proves physical erasure in a protected environment:
//! logical deletion of the record and erasure are different statements (Document 10).
//!
//! Table and decisions B-01/B-02 are the same as registration: the rule is one, and repeating
//! it in two places would be to separate in a third.

mod generation_matches;
mod removal_registry_action;

pub use crate::machines::enrollment::{
    MutationInput, MutationStep, MutationStepOutcome, effects_for, phase_for, step,
};

pub use generation_matches::generation_matches;
pub use removal_registry_action::removal_registry_action;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
