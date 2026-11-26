//! Verification of a separate channel of a trusted owner of hardware resources.
//!
//! Checking the shape and match of the area does not confirm the policy, source of the
//! message, stable fencing or origin of the descriptors.

mod error;
mod outcome;
mod prepare_validation;
mod quiescence;

pub use error::Error;
pub use outcome::Outcome;
pub use prepare_validation::validate_prepare;
pub use prepare_validation::validate_response;
pub use quiescence::QUIESCENCE_REQUEST_LIMIT;
pub use quiescence::Quiescence;
pub use quiescence::validate_quiescence_observation;
pub use quiescence::validate_quiescence_request;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
