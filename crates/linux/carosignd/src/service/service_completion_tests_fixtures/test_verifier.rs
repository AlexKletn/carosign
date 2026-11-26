use super::*;
use crate::operations::OperationRegistry;

pub(super) struct TestVerifier<'a> {
    pub(super) registry: &'a OperationRegistry,
    pub(super) clock: &'a carosign_adapters::clock::StepClock,
}

impl EvidencePort for TestVerifier<'_> {
    fn evaluate<'a>(
        &'a self,
        material: carosign_evidence::material::EvidenceMaterialRef,
        context: &'a EvidenceContext,
        requirements: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> BoxFuture<'a, Result<carosign_core::assurance::AssuranceAssessment, EvidenceRefusal>>
    {
        Box::pin(async move {
            if requirements.hardware_required {
                return Err(EvidenceRefusal::Unverified);
            }
            let lease = self
                .registry
                .take_actor_evidence(context.operation, material.clone(), self.clock.now())
                .map_err(|_| EvidenceRefusal::Unverified)?;
            if lease.bytes() != [42; 32] {
                return Err(EvidenceRefusal::Unverified);
            }
            lease
                .finish(self.clock.now())
                .map_err(|_| EvidenceRefusal::Unverified)?;
            Ok(carosign_core::assurance::AssuranceAssessment::record(
                EvidenceId::from_bytes(material.id()).unwrap(),
                carosign_core::assurance::AssuranceClass::SoftwareAsserted,
                context.operation,
                context.enrollment.id,
                context.authenticator.id,
            ))
        })
    }
    fn invalidate(
        &self,
        _: carosign_evidence::material::EvidenceMaterialRef,
    ) -> BoxFuture<'_, Result<(), EvidenceRefusal>> {
        Box::pin(async { Err(EvidenceRefusal::Unavailable) })
    }
}
