use super::{VerificationObservation, common};

/// The result of the exchange does not certify the level of security or user input.
pub enum Outcome {
    Observation(Box<VerificationObservation>),
    Rejected(common::Error),
}
