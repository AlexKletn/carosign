use super::{
    EffectOwner, OperationId, OperationRegistry, PositiveCompletionError, PositiveCompletionStage,
    Step,
};

/// Maintains reserve and retention of effect throughout the delivery.
/// Trust in connection and source of authority ensures the composition of the service.
#[allow(dead_code)]
pub(crate) struct PositiveCompletion<'a> {
    registry: &'a OperationRegistry,
    effects: &'a EffectOwner,
    pub(super) operation: OperationId,
    effect: carosign_engine::PlannedEffect,
    delivery: crate::operations::PositiveDeliveryOwner<'a>,
    claim: Option<crate::effect_owner::EffectClaim<'a>>,
    pub(in super::super) scoped_assessment:
        Option<carosign_runtime::ports::ScopedAssertionAssessment>,
    stage: PositiveCompletionStage,
}

#[allow(dead_code)]
impl<'a> PositiveCompletion<'a> {
    pub(crate) fn new(
        registry: &'a OperationRegistry,
        effects: &'a EffectOwner,
        operation: OperationId,
        caller: carosign_runtime::ports::CallerContext,
        effect: carosign_engine::PlannedEffect,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<Self, PositiveCompletionError> {
        let claim = effects
            .claim_effect(operation, effect)
            .map_err(PositiveCompletionError::Effect)?;
        let delivery = crate::operations::PositiveDeliveryOwner::reserve(
            registry, operation, caller, effect, now,
        )
        .map_err(PositiveCompletionError::Delivery)?;
        Ok(Self {
            registry,
            effects,
            operation,
            effect,
            delivery,
            claim: Some(claim),
            scoped_assessment: None,
            stage: PositiveCompletionStage::Sending,
        })
    }

    pub(crate) fn report_matched_enrollment(&mut self) {
        self.delivery.report_matched_enrollment = true;
        self.delivery.scoped_match = self
            .scoped_assessment
            .as_ref()
            .and_then(|assessment| assessment.assertion().matched);
    }

    /// When EAGAIN, the owner remains with the caller along with the cursor and claim.
    pub(crate) async fn drive(
        &mut self,
        connection: &crate::server::Connection,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<crate::operations::OwnerSendProgress, PositiveCompletionError> {
        use crate::operations::OwnerSendProgress;
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
        use carosign_runtime::OperationMsg;
        if self.stage == PositiveCompletionStage::Sending {
            if self
                .scoped_assessment
                .as_ref()
                .is_some_and(|a| !a.still_valid(clock.now()))
            {
                return Err(PositiveCompletionError::InvalidStage);
            }
            // Loss of the holder after the start of the call does not allow re-transfer of
            // the effect, even if the outcome of the recording is unknown.
            self.claim
                .as_mut()
                .ok_or(PositiveCompletionError::InvalidStage)?
                .mark_started();
            loop {
                match self
                    .delivery
                    .try_send_one(connection, authority, clock)
                    .await
                    .map_err(PositiveCompletionError::Send)?
                {
                    OwnerSendProgress::Advanced => continue,
                    OwnerSendProgress::WouldBlock => return Ok(OwnerSendProgress::WouldBlock),
                    OwnerSendProgress::Sent => {
                        self.claim
                            .take()
                            .ok_or(PositiveCompletionError::InvalidStage)?
                            .acknowledge();
                        self.stage = PositiveCompletionStage::Receipt;
                        break;
                    }
                }
            }
        }
        if self.stage == PositiveCompletionStage::Receipt {
            let step = {
                let mut guard = self
                    .registry
                    .checkout(self.operation)
                    .map_err(PositiveCompletionError::Operation)?;
                let actor = &mut guard.operation_mut().actor;
                actor
                    .post(OperationMsg::Domain(DomainInput::Receipt {
                        effect: self.effect,
                        receipt: EffectReceipt::OwnerDeliveryRecorded,
                    }))
                    .map_err(|_| PositiveCompletionError::InvalidStage)?;
                actor.run_once()
            };
            self.effects
                .accept_step(self.operation, &step)
                .map_err(PositiveCompletionError::Effect)?;
            self.stage = PositiveCompletionStage::Destroy;
        }
        if self.stage == PositiveCompletionStage::Destroy {
            // The recording of the statement shall be retained until confirmed dispatch.
            // Only now the one-time issue is released for withdrawal.
            drop(self.scoped_assessment.take());
            let effect = self.pending_effect(EffectSpec::DestroyEvidence)?;
            self.effects
                .acknowledge_step(self.operation)
                .map_err(PositiveCompletionError::Effect)?;
            self.effects
                .execute_destroy_evidence(self.registry, self.operation, effect)
                .map_err(PositiveCompletionError::Operation)?;
            self.stage = PositiveCompletionStage::DestroyHandoff;
        }
        if self.stage == PositiveCompletionStage::DestroyHandoff {
            self.registry
                .handoff_evidence_destroy(self.operation, self.effects)
                .map_err(PositiveCompletionError::Effect)?;
            self.stage = PositiveCompletionStage::Record;
        }
        if self.stage == PositiveCompletionStage::Record {
            let effect = self.pending_effect(EffectSpec::RecordTerminal)?;
            self.effects
                .acknowledge_step(self.operation)
                .map_err(PositiveCompletionError::Effect)?;
            self.effects
                .execute_delivered_record(self.registry, self.operation, effect)
                .map_err(PositiveCompletionError::Effect)?;
            self.stage = PositiveCompletionStage::RecordHandoff;
        }
        if self.stage == PositiveCompletionStage::RecordHandoff {
            self.effects
                .accept_record(self.operation)
                .map_err(PositiveCompletionError::Effect)?;
            self.stage = PositiveCompletionStage::Publish;
        }
        if self.stage == PositiveCompletionStage::Publish {
            self.registry
                .publish_delivered_verification(self.operation)
                .map_err(PositiveCompletionError::Delivery)?;
            self.stage = PositiveCompletionStage::Finished;
            return Ok(OwnerSendProgress::Sent);
        }
        Err(PositiveCompletionError::InvalidStage)
    }

    fn pending_effect(
        &self,
        expected: carosign_engine::EffectSpec,
    ) -> Result<carosign_engine::PlannedEffect, PositiveCompletionError> {
        match self.effects.step(self.operation) {
            Some(Step::Advanced { effects, .. }) => effects
                .into_iter()
                .find(|effect| effect.spec == expected)
                .ok_or(PositiveCompletionError::InvalidStage),
            _ => Err(PositiveCompletionError::InvalidStage),
        }
    }
}
