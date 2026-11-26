use super::{
    EffectOwner, MatchedEvidenceError, MatchedEvidenceStage, OperationId, OperationRegistry,
};

/// Holds the test of the material until the result is transmitted to the actor, the lost call
/// of the examiner does not repeat, its outcome remains uncertain.
#[allow(dead_code)]
pub(crate) struct MatchedEvidenceOwner {
    operation: OperationId,
    material: carosign_evidence::material::EvidenceMaterialRef,
    effect: carosign_engine::PlannedEffect,
    stage: MatchedEvidenceStage,
}

impl MatchedEvidenceOwner {
    pub(crate) fn new(
        operation: OperationId,
        material: carosign_evidence::material::EvidenceMaterialRef,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<Self, MatchedEvidenceError> {
        if material.operation() != *operation.as_bytes()
            || effect.spec != carosign_engine::EffectSpec::VerifyEvidence
        {
            return Err(MatchedEvidenceError::InvalidStage);
        }
        Ok(Self {
            operation,
            material,
            effect,
            stage: MatchedEvidenceStage::Ready,
        })
    }

    /// The verifier and the requirements belong to the composition of the service. This
    /// transition does not confirm the identity of the recipient and does not allow the
    /// sending of the result.
    pub(crate) async fn drive(
        &mut self,
        registry: &OperationRegistry,
        effects: &EffectOwner,
        verifier: &dyn carosign_runtime::ports::EvidencePort,
        requirements: &carosign_evidence::requirements::EvidenceRequirements,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), MatchedEvidenceError> {
        if self.stage == MatchedEvidenceStage::Ready {
            self.stage = MatchedEvidenceStage::Evaluating;
            effects
                .execute_evidence(
                    registry,
                    self.effect,
                    self.material.clone(),
                    *requirements,
                    verifier,
                    clock,
                )
                .await
                .map_err(MatchedEvidenceError::Operation)?;
            self.stage = MatchedEvidenceStage::Evaluated;
        }
        if self.stage == MatchedEvidenceStage::Evaluated {
            effects
                .apply_evidence(registry, self.operation, clock)
                .map_err(MatchedEvidenceError::Operation)?;
            self.stage = MatchedEvidenceStage::Applied;
        }
        if self.stage == MatchedEvidenceStage::Applied {
            registry
                .handoff_evidence_evaluation(self.operation, effects)
                .map_err(MatchedEvidenceError::Handoff)?;
            self.stage = MatchedEvidenceStage::HandedOff;
            return Ok(());
        }
        Err(MatchedEvidenceError::InvalidStage)
    }
}
