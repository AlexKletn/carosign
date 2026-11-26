use super::ProviderAssertionRecord;
use zeroize::Zeroizing;

pub(super) enum SlotState {
    Empty,
    NeverIssued,
    Held(Zeroizing<Vec<u8>>),
    /// The provider’s approval does not contain byte material.
    HeldAssertion(Box<ProviderAssertionRecord>),
    Checking,
    RevokedInUse,
    Destroyed,
}
