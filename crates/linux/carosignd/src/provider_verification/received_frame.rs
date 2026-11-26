use super::{Payload, ProviderFrame};

/// The time frame does not hold a copy of the token after parsing or rejection.
pub(super) struct ReceivedFrame(pub(super) ProviderFrame);

impl Drop for ReceivedFrame {
    fn drop(&mut self) {
        use carosign_protocol::carosign::provider::v1::verification_outcome::Evidence;
        use zeroize::Zeroize;
        if let Some(Payload::VerificationOutcome(outcome)) = &mut self.0.payload
            && let Some(Evidence::OpaqueToken(token)) = &mut outcome.evidence
        {
            token.zeroize();
        }
    }
}
