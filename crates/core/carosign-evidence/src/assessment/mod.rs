//! Evaluation: The result of the examiner and its origin.
//!
//! The assessment comes from neither the manifest nor the worker.
//! It appears only in the verifier, and it is written in what it was obtained (document 11).

mod assessment;
mod provenance;

pub use assessment::Assessment;
pub use provenance::Provenance;
