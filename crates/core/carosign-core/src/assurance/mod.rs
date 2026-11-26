//! Guarantees: what is verified and within what limits.
//!
//! ⚠️ The separation of the three layers is the main thing in the whole crayte (Document 11):
//!
//! 1. **Coincidence** - Approval by the provider (`Matched`), may lie; 2. **evidence** is the
//! material tied to the operation; 3. **accepted factor** is the conclusion of the
//! independent verifier.
//!
//! Here are described only the second and third: what exactly proved and how much.
//! The assessment comes from neither the manifest nor the worker.

mod assurance_assessment;
mod assurance_class;

pub use assurance_assessment::AssuranceAssessment;
pub use assurance_class::AssuranceClass;

#[path = "mod.tests.rs"]
#[cfg(test)]
mod tests;
