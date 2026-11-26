use super::{
    BoxFuture, EvidenceContext, EvidencePort, EvidenceRefusal, EvidenceRequirements,
    ScopedAssertionAssessment, ScopedAssertionContext, ScopedProviderAssertion, Sources, evaluate,
};

/// Evaluation port with explicitly transmitted independent system sources.
/// The designer himself does not certify the source and does not connect it to the service.
/// Byte material is not evaluated here: a separate port is needed for it.
pub(crate) struct ScopedEvidencePort<'a> {
    pub(crate) sources: Sources<'a>,
}

impl EvidencePort for ScopedEvidencePort<'_> {
    fn evaluate_scoped_assertion<'a>(
        &'a self,
        assertion: ScopedProviderAssertion,
        context: &'a ScopedAssertionContext,
        requirements: &'a EvidenceRequirements,
    ) -> BoxFuture<'a, Result<ScopedAssertionAssessment, EvidenceRefusal>> {
        Box::pin(async move { evaluate(assertion, context, requirements, &self.sources) })
    }

    fn evaluate<'a>(
        &'a self,
        _material: carosign_evidence::material::EvidenceMaterialRef,
        _context: &'a EvidenceContext,
        _requirements: &'a EvidenceRequirements,
    ) -> BoxFuture<'a, Result<carosign_core::assurance::AssuranceAssessment, EvidenceRefusal>>
    {
        Box::pin(async { Err(EvidenceRefusal::Unsupported) })
    }

    fn invalidate(
        &self,
        material: carosign_evidence::material::EvidenceMaterialRef,
    ) -> BoxFuture<'_, Result<(), EvidenceRefusal>> {
        Box::pin(async move {
            self.sources
                .store
                .invalidate(material)
                .map_err(|_| EvidenceRefusal::Unavailable)
        })
    }
}
