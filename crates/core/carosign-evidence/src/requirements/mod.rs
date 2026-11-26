//! Proof Requirements: What must be verified.
//!
//! ⚠️ It's the request of the * * who asks*, not the provider: `Matched` It can't expand the
//! list. It's an unknown property. `Unverified` Rejection for any policy that requires it
//! (Document 11).

mod evidence_requirements;
mod requirement_conflict;

pub use evidence_requirements::EvidenceRequirements;
pub use requirement_conflict::RequirementConflict;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
