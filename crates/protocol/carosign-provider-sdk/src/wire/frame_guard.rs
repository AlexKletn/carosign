use carosign_protocol::carosign::provider::v1::ProviderFrame;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
use zeroize::Zeroize;

pub(super) struct FrameGuard(pub(super) ProviderFrame);

impl FrameGuard {
    pub(super) fn take(mut self) -> ProviderFrame {
        std::mem::take(&mut self.0)
    }

    pub(super) fn clear(&mut self) {
        use carosign_protocol::carosign::provider::v1::verification_outcome::Evidence;
        if let Some(Payload::VerificationOutcome(outcome)) = &mut self.0.payload
            && let Some(Evidence::OpaqueToken(token)) = &mut outcome.evidence
        {
            token.zeroize();
        }
    }
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        self.clear();
    }
}
