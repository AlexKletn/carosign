use carosign_evidence::material::EvidenceMaterialRef;
use carosign_evidence::requirements::EvidenceRequirements;
use carosign_evidence::verifier::Observed;
use carosign_evidence::verifier::Verdict;
use carosign_runtime::ports::EvidenceContext;

/// What a tester can do.
///
/// ⚠️ The trait is narrow: it accepts **links and assertions**, not bytes.
/// The decision on the class of guarantee remains with the inspector.
pub trait EvidenceCheck: Send + Sync {
    /// Checking.
    fn check(
        &self,
        material: EvidenceMaterialRef,
        context: &EvidenceContext,
        requirements: &EvidenceRequirements,
    ) -> Verdict;
    /// Observation of the world around (receiver, call, hardware path).
    fn observed(&self) -> Observed;
}
