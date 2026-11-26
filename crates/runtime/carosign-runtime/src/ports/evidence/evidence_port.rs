use super::super::BoxFuture;
use super::EvidenceContext;
use super::EvidenceRefusal;
use super::ScopedAssertionAssessment;
use super::ScopedAssertionContext;
use carosign_core::assurance::AssuranceAssessment;

pub trait EvidencePort: Send + Sync {
    /// Once passes a session-restricted statement to an independent evaluation, the old
    /// examiner does not confirm its origin and is obliged to refuse; the approval itself
    /// does not constitute an entry permit.
    fn evaluate_scoped_assertion<'a>(
        &'a self,
        _assertion: crate::provider::session::ScopedProviderAssertion,
        _context: &'a ScopedAssertionContext,
        _requirements: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> BoxFuture<'a, Result<ScopedAssertionAssessment, EvidenceRefusal>> {
        Box::pin(async { Err(EvidenceRefusal::Unavailable) })
    }
    /// Verifies the retained material in full context independently.
    fn evaluate<'a>(
        &'a self,
        material: carosign_evidence::material::EvidenceMaterialRef,
        context: &'a EvidenceContext,
        requirements: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> BoxFuture<'a, Result<AssuranceAssessment, EvidenceRefusal>>;

    /// Confirms the withdrawal only after the release of the material by the owner.
    fn invalidate(
        &self,
        material: carosign_evidence::material::EvidenceMaterialRef,
    ) -> BoxFuture<'_, Result<(), EvidenceRefusal>>;
}
