use super::*;

pub(super) struct ScopedVerifier<'a> {
    pub(super) store: &'a carosign_evidence::store::EvidenceStore,
    pub(super) clock: &'a dyn TimeSource,
    pub(super) challenges: &'a crate::scoped_challenge::ScopedChallenges,
    pub(super) expected_operation: OperationId,
    pub(super) expected_worker: WorkerInstanceId,
    pub(super) expected_command: CommandId,
}

impl crate::scoped_verifier::ScopedTrust for ScopedVerifier<'_> {
    fn confirm(
        &self,
        assertion: &carosign_runtime::provider::session::ScopedProviderAssertion,
        context: &carosign_runtime::ports::ScopedAssertionContext,
    ) -> Result<(), EvidenceRefusal> {
        // Fixture only: The ID comparison does not confirm the provider's system origin or
        // recipient's policy.
        if assertion.operation() != self.expected_operation
            || assertion.worker() != self.expected_worker
            || assertion.command() != self.expected_command
            || assertion.epoch() != context.authenticator.device_epoch
        {
            return Err(EvidenceRefusal::Unverified);
        }
        Ok(())
    }
}

impl EvidencePort for ScopedVerifier<'_> {
    fn evaluate_scoped_assertion<'a>(
        &'a self,
        assertion: carosign_runtime::provider::session::ScopedProviderAssertion,
        context: &'a carosign_runtime::ports::ScopedAssertionContext,
        requirements: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> BoxFuture<'a, Result<carosign_runtime::ports::ScopedAssertionAssessment, EvidenceRefusal>>
    {
        Box::pin(async move {
            let port = crate::scoped_verifier::ScopedEvidencePort {
                sources: crate::scoped_verifier::Sources {
                    trust: self,
                    challenges: self.challenges,
                    store: self.store,
                    ids: &Ids,
                    clock: self.clock,
                },
            };
            port.evaluate_scoped_assertion(assertion, context, requirements)
                .await
        })
    }
    fn evaluate<'a>(
        &'a self,
        _: carosign_evidence::material::EvidenceMaterialRef,
        _: &'a EvidenceContext,
        _: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> BoxFuture<'a, Result<carosign_core::assurance::AssuranceAssessment, EvidenceRefusal>>
    {
        Box::pin(async { Err(EvidenceRefusal::Unavailable) })
    }
    fn invalidate(
        &self,
        _: carosign_evidence::material::EvidenceMaterialRef,
    ) -> BoxFuture<'_, Result<(), EvidenceRefusal>> {
        Box::pin(async { Err(EvidenceRefusal::Unavailable) })
    }
}
