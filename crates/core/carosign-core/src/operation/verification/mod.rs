//! Verification: what was asked and how it ended.
//!
//! `Matched` carries`EvidenceId`](crate::ids::EvidenceId) and **remains the outcome of the
//! provider: it does not become either an accepted factor or an entry success (invariant 4).

mod accepted_factor;
mod verification_failure;
mod verification_outcome;
mod verification_result;
mod verify_request;

pub use accepted_factor::AcceptedFactor;
pub use verification_failure::VerificationFailure;
pub use verification_outcome::VerificationOutcome;
pub use verification_result::VerificationResult;
pub use verify_request::VerifyRequest;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
