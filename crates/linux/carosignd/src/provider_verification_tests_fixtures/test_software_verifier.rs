use super::*;

pub(super) struct TestSoftwareVerifier<'a> {
    pub(super) registry: &'a crate::operations::OperationRegistry,
    pub(super) clock: &'a carosign_adapters::clock::StepClock,
    pub(super) wrong_assessment: bool,
    pub(super) interrupted: bool,
    pub(super) revoke_during: bool,
}

impl carosign_runtime::ports::EvidencePort for TestSoftwareVerifier<'_> {
    fn evaluate<'a>(
        &'a self,
        material: carosign_evidence::material::EvidenceMaterialRef,
        context: &'a carosign_runtime::ports::EvidenceContext,
        requirements: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> carosign_runtime::ports::BoxFuture<
        'a,
        Result<
            carosign_core::assurance::AssuranceAssessment,
            carosign_runtime::ports::EvidenceRefusal,
        >,
    > {
        Box::pin(async move {
            use carosign_runtime::ports::{EvidenceRefusal, TimeSource};
            if self.interrupted {
                std::future::pending::<()>().await;
            }
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
            if self.revoke_during {
                assert!(self.registry.revoke_verification(context.operation).is_ok());
            }
            lease
                .finish(self.clock.now())
                .map_err(|_| EvidenceRefusal::Unverified)?;
            Ok(carosign_core::assurance::AssuranceAssessment::record(
                carosign_core::ids::EvidenceId::from_bytes(material.id()).unwrap(),
                carosign_core::assurance::AssuranceClass::SoftwareAsserted,
                context.operation,
                if self.wrong_assessment {
                    EnrollmentId::from_bytes([98; 16]).unwrap()
                } else {
                    context.enrollment.id
                },
                context.authenticator.id,
            ))
        })
    }
    fn invalidate(
        &self,
        _: carosign_evidence::material::EvidenceMaterialRef,
    ) -> carosign_runtime::ports::BoxFuture<
        '_,
        Result<(), carosign_runtime::ports::EvidenceRefusal>,
    > {
        Box::pin(async { Err(carosign_runtime::ports::EvidenceRefusal::Unavailable) })
    }
}
