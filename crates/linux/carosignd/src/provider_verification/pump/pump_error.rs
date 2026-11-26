use super::VerificationError;

/// Error reading or transmission; busy observation is saved for repetition.
#[derive(Debug)]
pub enum PumpError {
    /// The channel refused or violated protocol.
    Exchange(VerificationError),
    /// The table didn't accept observation.
    Delivery(crate::operations::ObservationFailure),
    /// The previously reported observation was rejected by the actor.
    Failed,
}
