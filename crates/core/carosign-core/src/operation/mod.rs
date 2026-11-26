//! Operations: request, outcome, cancellation.
//!
//! ⚠️ Three things that are **separated here and don't merge:
//!
//! * request - what has been requested`VerifyRequest` and relatives;
//! * The outcome is the end result for the user ([]`VerificationOutcome`],
//!   [`MutationOutcome`]);
//! * Acceptance - does it confer a right`ValidatedGrant` and `AssuranceAssessment`).
//!
//! [`VerificationOutcome::Matched`** Outcome of the provider, not success: it refers to
//! evidence that has yet to be verified (C-01/E-01).
//! [`MutationOutcome::NeedsReconciliation`] cannot be rewritten in `NoChange`/`Cancelled`:
//! These are different claims about hardware.

pub mod cancellation;
pub mod context;
pub mod mutation;
pub mod plan;
pub mod verification;

pub use cancellation::*;
pub use context::*;
pub use mutation::*;
pub use verification::*;
