use super::ExchangeError;

/// Disconnection or context inconsistency.
#[derive(Debug)]
pub enum VerificationError {
    /// The channel refused; the result of the shipment may not be known.
    Transport(ExchangeError),
    /// Incorrect frame, phase, area, or sequence.
    Protocol,
    /// The exchange is complete.
    Finished,
}
