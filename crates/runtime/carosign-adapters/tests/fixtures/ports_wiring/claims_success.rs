use carosign_adapters::evidence::EvidenceCheck;
use carosign_evidence::verifier::{Observed, ProtectedAnswer, Verdict};

pub(super) struct ClaimsSuccess;

impl EvidenceCheck for ClaimsSuccess {
    fn check(
        &self,
        _: carosign_evidence::material::EvidenceMaterialRef,
        _: &carosign_runtime::ports::EvidenceContext,
        _: &carosign_evidence::requirements::EvidenceRequirements,
    ) -> Verdict {
        Verdict::Accepted(carosign_evidence::assessment::Assessment::hardware())
    }
    fn observed(&self) -> Observed {
        Observed {
            recipient: [1; 16],
            challenge: [2; 16],
            hardware_path: true,
            protected: ProtectedAnswer::Verified,
        }
    }
}
