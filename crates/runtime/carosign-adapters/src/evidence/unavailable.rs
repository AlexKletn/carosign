use super::evidence_check::EvidenceCheck;
use carosign_evidence::material::EvidenceMaterialRef;
use carosign_evidence::requirements::EvidenceRequirements;
use carosign_evidence::verifier::Observed;
use carosign_evidence::verifier::Verdict;
use carosign_runtime::ports::EvidenceContext;

/// The checker's cover is "all unavailable."
///
/// ⚠️ Such silence is the only safe: while there is no inspector, no assessment is issued.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unavailable;

impl EvidenceCheck for Unavailable {
    fn check(
        &self,
        _material: EvidenceMaterialRef,
        _context: &EvidenceContext,
        _requirements: &EvidenceRequirements,
    ) -> Verdict {
        Verdict::Rejected(carosign_evidence::material::EvidenceError::VerifierUnavailable)
    }

    fn observed(&self) -> Observed {
        Observed {
            recipient: [0u8; 16],
            challenge: [0u8; 16],
            hardware_path: false,
            protected: carosign_evidence::verifier::ProtectedAnswer::Unavailable,
        }
    }
}
