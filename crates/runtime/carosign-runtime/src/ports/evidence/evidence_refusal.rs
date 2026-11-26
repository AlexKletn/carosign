/// The verifier's refusal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EvidenceRefusal {
    /// The format is not supported.
    Unsupported,
    /// Origin is not established.
    Unverified,
    WrongRecipient,
    Expired,
    /// Checker's not available.
    Unavailable,
}
