use super::VerificationFailure;

/// What happened to the user after the test and the decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationResult {
    /// Factor accepted ** for this ** request (not login).
    AcceptedFactor,
    NotMatched,
    /// Couldn't get it off.
    NoSample,
    Cancelled,
    /// The machine's unavailable.
    Unavailable,
    /// Refusal.
    Failed(VerificationFailure),
}
