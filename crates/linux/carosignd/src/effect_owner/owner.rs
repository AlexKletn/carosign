use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;

use carosign_core::ids::OperationId;
use carosign_engine::{EffectSpec, PlannedEffect};
use carosign_runtime::operation::actor::Step;

use crate::cleanup_owner::{CleanupClaim, CleanupOwner, CleanupResult};
use crate::control_watch::{ControlClaim, WatchResult};
use crate::operations::{CleanupProgress, OperationRegistry};
use crate::resources::TrustedCleanupBroker;

use super::{
    DriveError, DriveOutcome, EffectClaim, EffectOwnerError, RecordClaim, RecordResult,
    StepPublishError, batch::Batch, effect_slot::EffectSlot, effect_state::EffectState,
    evidence_evaluation::EvidenceEvaluation, evidence_evaluation_slot::EvidenceEvaluationSlot,
    record_slot::RecordSlot,
};

const BATCHES_MAX: usize = 64;

/// Possesses step and effects until explicit transfer to performers.
#[derive(Default)]
pub(crate) struct EffectOwner {
    pub(super) batches: Mutex<BTreeMap<OperationId, Batch>>,
    cleanup: CleanupOwner,
    pub(super) records: Mutex<BTreeMap<OperationId, RecordSlot>>,
    notifications: Mutex<BTreeMap<OperationId, Vec<u32>>>,
    evidence_evaluations: Mutex<BTreeMap<OperationId, EvidenceEvaluationSlot>>,
}

impl EffectOwner {
    /// Rechecks the owner after the evaluation. If you fail or interrupt, the effect remains
    /// waiting; no confirmation of the result is issued here.
    #[allow(dead_code)]
    pub(crate) async fn revalidate_evaluated_owner(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), crate::operations::ObservationFailure> {
        if effect.spec != EffectSpec::RevalidateOwner {
            return Err(crate::operations::ObservationFailure::ContextMismatch);
        }
        let claim = self
            .claim_effect(id, effect)
            .map_err(|_| crate::operations::ObservationFailure::Busy)?;
        registry
            .revalidate_evaluated_owner(id, effect, authority, clock)
            .await?;
        claim.acknowledge();
        Ok(())
    }

    /// Checks the owner after a confirmed cleaning before delivery.
    #[allow(dead_code)]
    pub(crate) async fn revalidate_cleaned_owner(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), crate::operations::ObservationFailure> {
        if effect.spec != EffectSpec::RevalidateOwner {
            return Err(crate::operations::ObservationFailure::ContextMismatch);
        }
        let claim = self
            .claim_effect(id, effect)
            .map_err(|_| crate::operations::ObservationFailure::Busy)?;
        registry
            .revalidate_cleaned_owner(id, effect, authority, clock)
            .await?;
        claim.acknowledge();
        Ok(())
    }

    /// The checker is selected by the line-up, and the lost call remains uncertain and does
    /// not automatically recur.
    #[allow(dead_code)]
    pub(crate) async fn execute_evidence(
        &self,
        registry: &OperationRegistry,
        effect: PlannedEffect,
        material: carosign_evidence::material::EvidenceMaterialRef,
        requirements: carosign_evidence::requirements::EvidenceRequirements,
        verifier: &dyn carosign_runtime::ports::EvidencePort,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), crate::operations::ObservationFailure> {
        use crate::operations::ObservationFailure;
        let id = OperationId::from_bytes(material.operation())
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        if effect.spec != EffectSpec::VerifyEvidence {
            return Err(ObservationFailure::ContextMismatch);
        }
        let context = registry.matched_evidence_context(id, &material, clock.now())?;
        if requirements.validate().is_err() || requirements.purpose != context.purpose {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut claim = self
            .claim_effect(id, effect)
            .map_err(|_| ObservationFailure::Busy)?;
        {
            let mut checks = self
                .evidence_evaluations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if checks.contains_key(&id) || checks.len() >= BATCHES_MAX {
                return Err(ObservationFailure::Busy);
            }
            checks.insert(id, EvidenceEvaluationSlot::Running);
        }
        claim.mark_started();
        let result = verifier
            .evaluate(material.clone(), &context, &requirements)
            .await;
        let record = EvidenceEvaluation {
            effect,
            material,
            context,
            requirements,
            result,
        };
        self.evidence_evaluations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(id, EvidenceEvaluationSlot::Ready(Box::new(record)));
        claim.acknowledge();
        Ok(())
    }

    /// The result is saved when the actor is busy or the transmission is refused.
    #[allow(dead_code)]
    pub(crate) fn apply_evidence(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), crate::operations::ObservationFailure> {
        use crate::operations::ObservationFailure;
        let record = {
            let checks = self
                .evidence_evaluations
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match checks.get(&id) {
                Some(EvidenceEvaluationSlot::Ready(record)) => record.as_ref().clone(),
                _ => return Err(ObservationFailure::Busy),
            }
        };
        let accepted = record.result.as_ref().is_ok_and(|assessment| {
            assessment.evidence().as_bytes() == &record.material.id()
                && assessment.operation() == record.context.operation
                && assessment.enrollment() == record.context.enrollment.id
                && assessment.authenticator() == record.context.authenticator.id
                && record.requirements.accepts(assessment.class())
        });
        registry.apply_evidence_evaluation(
            record.effect,
            &record.material,
            &record.context,
            accepted,
            clock.now(),
        )?;
        self.evidence_evaluations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&id);
        Ok(())
    }

    /// Leads single effects of cleaning, destruction and fixation; the rest preserves.
    /// A positive outcome is recorded only after the confirmed dispatch, removal and
    /// destruction of the material checked by the registry.
    /// The timer and the mediator provide a trusted platform composition.
    #[cfg(test)]
    pub async fn drive_negative<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        broker: &dyn TrustedCleanupBroker,
        timeout: impl Fn() -> T,
    ) -> Result<DriveOutcome, DriveError> {
        self.drive_negative_inner(registry, id, None, broker, timeout)
            .await
    }

    /// Cleaning is used by the same scheduler who issued the resources.
    pub(crate) async fn drive_negative_scheduled<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn TrustedCleanupBroker,
        timeout: impl Fn() -> T,
    ) -> Result<DriveOutcome, DriveError> {
        self.drive_negative_inner(registry, id, Some(scheduler), broker, timeout)
            .await
    }

    async fn drive_negative_inner<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        scheduler: Option<&Mutex<carosign_runtime::scheduler::Scheduler>>,
        broker: &dyn TrustedCleanupBroker,
        timeout: impl Fn() -> T,
    ) -> Result<DriveOutcome, DriveError> {
        let hub = registry.event_hub(id).map_err(DriveError::Operation)?;
        for _ in 0..4 {
            let Some(Step::Advanced { state, effects }) = self.step(id) else {
                return Ok(DriveOutcome::ExternalEffect);
            };
            if state.common.phase.is_terminal() {
                return Ok(DriveOutcome::TerminalPending);
            }
            if !self.step_published(id) {
                let mut hub = hub.lock().unwrap_or_else(|error| error.into_inner());
                self.publish_step_retained(id, &mut hub)
                    .map_err(DriveError::Publish)?;
            }
            if effects.len() != 1 {
                return Ok(DriveOutcome::ExternalEffect);
            }
            let effect = effects[0];
            match effect.spec {
                EffectSpec::ReleaseResources => {
                    if let Some(scheduler) = scheduler {
                        self.execute_scheduled_cleanup(
                            registry,
                            id,
                            effect,
                            scheduler,
                            broker,
                            timeout(),
                        )
                        .await
                        .map_err(DriveError::Effect)?;
                    } else {
                        #[cfg(test)]
                        self.execute_cleanup(registry, id, effect, broker, timeout())
                            .await
                            .map_err(DriveError::Effect)?;
                        #[cfg(not(test))]
                        return Ok(DriveOutcome::ExternalEffect);
                    }
                    let claim = self
                        .claim_cleanup(id)
                        .ok_or(DriveError::Effect(EffectOwnerError::NotPending))?;
                    if scheduler.is_some() {
                        self.accept_scheduled_cleanup(claim)
                            .map_err(DriveError::Effect)?;
                    } else {
                        self.accept_cleanup(claim).map_err(DriveError::Effect)?;
                    }
                }
                EffectSpec::RecordTerminal => {
                    if state.common.terminal
                        == Some(carosign_engine::TerminalOutcome::FactorAccepted)
                    {
                        // This branch does not confirm shipment: the registry separately
                        // requires Sent, cleaning and destruction of the material.
                        self.execute_delivered_record(registry, id, effect)
                            .map_err(DriveError::Effect)?;
                    } else {
                        self.execute_record(registry, id, effect)
                            .map_err(DriveError::Effect)?;
                    }
                    self.accept_record(id).map_err(DriveError::Effect)?;
                }
                EffectSpec::DestroyEvidence => {
                    self.execute_destroy_evidence(registry, id, effect)
                        .map_err(DriveError::Operation)?;
                    registry
                        .handoff_evidence_destroy(id, self)
                        .map_err(DriveError::Effect)?;
                }
                _ => return Ok(DriveOutcome::ExternalEffect),
            }
        }
        Ok(DriveOutcome::StepLimit)
    }
    fn step_published(&self, id: OperationId) -> bool {
        self.batches
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)
            .is_some_and(|batch| batch.step_accepted)
    }
    /// Publishes a non-terminal step and confirms it only after success.
    /// A terminal outcome requires a separate authorization of the recipient.
    fn publish_step(
        &self,
        id: OperationId,
        hub: &mut carosign_runtime::operation::hub::EventHub,
    ) -> Result<Vec<u32>, StepPublishError> {
        use carosign_runtime::operation::events::EventKind;
        let mut batches = self
            .batches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let batch = batches.get_mut(&id).ok_or(StepPublishError::Missing)?;
        if hub.operation() != id {
            return Err(StepPublishError::WrongOperation);
        }
        let Step::Advanced { state, .. } = &batch.step else {
            return Err(StepPublishError::Missing);
        };
        if state.common.phase.is_terminal() {
            return Err(StepPublishError::TerminalRequiresAuthorization);
        }
        if batch.step_accepted {
            return Err(StepPublishError::Missing);
        }
        let disconnected = hub
            .publish(state.common.phase, EventKind::StateChanged)
            .map_err(StepPublishError::Event)?;
        batch.step_accepted = true;
        if batch.effects.is_empty() {
            batches.remove(&id);
        }
        Ok(disconnected)
    }

    /// Stores notifications until handed over to the owner of the connections.
    fn publish_step_retained(
        &self,
        id: OperationId,
        hub: &mut carosign_runtime::operation::hub::EventHub,
    ) -> Result<(), StepPublishError> {
        use carosign_runtime::operation::observer::OBSERVERS_MAX;
        const NOTIFICATIONS_MAX: usize = 128;
        let mut pending = self
            .notifications
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if (!pending.contains_key(&id) && pending.len() >= BATCHES_MAX)
            || pending.get(&id).map_or(0, Vec::len) + OBSERVERS_MAX > NOTIFICATIONS_MAX
        {
            return Err(StepPublishError::NotificationLimit);
        }
        let disconnected = self.publish_step(id, hub)?;
        if !disconnected.is_empty() {
            pending.entry(id).or_default().extend(disconnected);
        }
        Ok(())
    }

    /// It only takes notifications after the owner is ready to process them.
    pub fn take_disconnected(&self, id: OperationId) -> Vec<u32> {
        self.notifications
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&id)
            .unwrap_or_default()
    }
    /// Atomically retains the step before confirming the receipt to the observer.
    /// In case of any failure, the holder returns the original result to the observer.
    pub fn accept_control(&self, claim: ControlClaim<'_>) -> Result<(), EffectOwnerError> {
        let id = claim.operation();
        let WatchResult::Step(step) = claim.result() else {
            return Err(EffectOwnerError::UnsupportedControl);
        };
        self.accept_step(id, step)?;
        claim.acknowledge();
        Ok(())
    }

    /// It promotes the control that is postponed in the occupied actor, confirms the observer
    /// only after saving the resulting step and its effects.
    pub fn accept_deferred_control(
        &self,
        registry: &OperationRegistry,
        claim: ControlClaim<'_>,
    ) -> Result<(), crate::operations::DeferredControlFailure> {
        if !matches!(claim.result(), WatchResult::Deferred) {
            return Err(crate::operations::DeferredControlFailure::NoControl);
        }
        registry.transfer_deferred_control(claim.operation(), self)?;
        claim.acknowledge();
        Ok(())
    }

    /// Save the cleaning step until the result is confirmed.
    /// When the queue is busy, the holder returns the cleaning result back.
    pub fn accept_cleanup(&self, claim: CleanupClaim<'_>) -> Result<(), EffectOwnerError> {
        let id = claim.operation();
        let step = match claim.result() {
            CleanupResult::Completed(Ok(
                CleanupProgress::Confirmed(step) | CleanupProgress::Uncertain(step),
            )) => step,
            _ => return Err(EffectOwnerError::UnsupportedCleanup),
        };
        self.accept_step(id, step)?;
        claim.acknowledge();
        Ok(())
    }

    /// Preserves the step generated by the actor and all its effects atomically.
    /// When the initial step is rejected, the caller remains.
    pub fn accept_step(&self, id: OperationId, step: &Step) -> Result<(), EffectOwnerError> {
        let step @ Step::Advanced { state, effects } = step else {
            return Err(EffectOwnerError::UnsupportedControl);
        };
        if state.common.context.operation_id != id {
            return Err(EffectOwnerError::UnsupportedControl);
        }
        if effects
            .iter()
            .enumerate()
            .any(|(index, effect)| effects[..index].contains(effect))
        {
            return Err(EffectOwnerError::UnsupportedControl);
        }
        let batch = Batch {
            step: step.clone(),
            step_accepted: false,
            effects: effects
                .iter()
                .copied()
                .map(|effect| EffectSlot {
                    effect,
                    state: EffectState::Pending,
                })
                .collect(),
        };
        {
            let mut batches = self
                .batches
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if batches.contains_key(&id) {
                return Err(EffectOwnerError::Duplicate);
            }
            if batches.len() >= BATCHES_MAX {
                return Err(EffectOwnerError::Limit);
            }
            batches.insert(id, batch);
        }
        Ok(())
    }

    /// A copy of the step for publication; this is neither right nor proof of delivery.
    pub fn step(&self, id: OperationId) -> Option<Step> {
        self.batches
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)
            .map(|batch| batch.step.clone())
    }

    /// Confirms the transfer of the step to the event owner.
    pub fn acknowledge_step(&self, id: OperationId) -> Result<(), EffectOwnerError> {
        let mut batches = self
            .batches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let batch = batches.get_mut(&id).ok_or(EffectOwnerError::NotPending)?;
        batch.step_accepted = true;
        if batch.effects.is_empty() {
            batches.remove(&id);
        }
        Ok(())
    }

    /// It takes effect to be passed on to another owner. You can't call the external system
    /// through that holder, and before the operation begins, the designated performer has to
    /// accept the outcome and the uncertainty.
    /// If the holder is lost before the transfer, the effect is again available.
    pub fn claim_effect(
        &self,
        id: OperationId,
        effect: PlannedEffect,
    ) -> Result<EffectClaim<'_>, EffectOwnerError> {
        let mut batches = self
            .batches
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let slot = batches
            .get_mut(&id)
            .and_then(|batch| batch.effects.iter_mut().find(|slot| slot.effect == effect))
            .ok_or(EffectOwnerError::NotPending)?;
        match slot.state {
            EffectState::Pending => slot.state = EffectState::Claimed,
            EffectState::Claimed => return Err(EffectOwnerError::Busy),
            EffectState::Uncertain => return Err(EffectOwnerError::Uncertain),
        }
        Ok(EffectClaim {
            owner: self,
            id,
            effect,
            finished: false,
            started: false,
        })
    }

    /// Passes the waiting cleaning to a limited owner, and after success, the results are
    /// stored there; the rest of the effects of this step do not disappear.
    #[cfg(test)]
    pub async fn execute_cleanup<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
    ) -> Result<(), EffectOwnerError> {
        if effect.spec != EffectSpec::ReleaseResources {
            return Err(EffectOwnerError::NotPending);
        }
        let mut claim = self.claim_effect(id, effect)?;
        claim.mark_started();
        self.cleanup
            .execute(registry, id, effect, broker, timeout)
            .await
            .map_err(EffectOwnerError::Cleanup)?;
        claim.acknowledge();
        Ok(())
    }

    /// Fenced cleaning; the results of the issuance of the queue remain with its owner.
    #[allow(dead_code)]
    pub(crate) async fn execute_scheduled_cleanup<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
    ) -> Result<(), EffectOwnerError> {
        if effect.spec != EffectSpec::ReleaseResources {
            return Err(EffectOwnerError::NotPending);
        }
        let mut claim = self.claim_effect(id, effect)?;
        claim.mark_started();
        self.cleanup
            .execute_scheduled(registry, id, effect, scheduler, broker, timeout)
            .await
            .map_err(EffectOwnerError::Cleanup)?;
        claim.acknowledge();
        Ok(())
    }

    /// Fenced cleaning, taking into account the time at the next issue.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn execute_scheduled_cleanup_at<T: Future<Output = ()>>(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn TrustedCleanupBroker,
        timeout: T,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), EffectOwnerError> {
        if effect.spec != EffectSpec::ReleaseResources {
            return Err(EffectOwnerError::NotPending);
        }
        let mut claim = self.claim_effect(id, effect)?;
        claim.mark_started();
        self.cleanup
            .execute_scheduled_at(registry, id, effect, scheduler, broker, timeout, clock)
            .await
            .map_err(EffectOwnerError::Cleanup)?;
        claim.acknowledge();
        Ok(())
    }

    /// Takes a step; issuing queues remain in the scheduler until received.
    #[allow(dead_code)]
    pub(crate) fn accept_scheduled_cleanup(
        &self,
        claim: CleanupClaim<'_>,
    ) -> Result<(), EffectOwnerError> {
        let id = claim.operation();
        let result = match claim.result() {
            CleanupResult::Scheduled(Ok(result)) => result,
            CleanupResult::Scheduled(Err(error)) => {
                return Err(EffectOwnerError::ScheduledCleanup(*error));
            }
            _ => return Err(EffectOwnerError::UnsupportedCleanup),
        };
        let step = match &result.progress {
            CleanupProgress::Confirmed(step) | CleanupProgress::Uncertain(step) => step,
        };
        self.accept_step(id, step)?;
        claim.acknowledge();
        Ok(())
    }

    /// Destroys the material through its sole owner, and the actor's transition remains on
    /// the registry until the next step is handed over separately.
    #[allow(dead_code)]
    pub(crate) fn execute_destroy_evidence(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
    ) -> Result<(), crate::operations::ObservationFailure> {
        if effect.spec != EffectSpec::DestroyEvidence {
            return Err(crate::operations::ObservationFailure::ContextMismatch);
        }
        let claim = self
            .claim_effect(id, effect)
            .map_err(|_| crate::operations::ObservationFailure::Busy)?;
        registry.destroy_actor_evidence(id, effect)?;
        claim.acknowledge();
        Ok(())
    }

    /// Additionally, it blocks issuance in the local scheduler without removing the effect:
    /// one scheduler does not prove a trusted quarantine of the broker.
    pub(crate) fn block_uncertain_scheduler_lease(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        scheduler: &mut carosign_runtime::scheduler::Scheduler,
    ) -> Result<(), crate::operations::ObservationFailure> {
        if effect.spec != EffectSpec::QuarantineScope {
            return Err(crate::operations::ObservationFailure::ContextMismatch);
        }
        let _claim = self
            .claim_effect(id, effect)
            .map_err(|_| crate::operations::ObservationFailure::Busy)?;
        registry.block_uncertain_scheduler_lease(id, effect, scheduler)
    }

    /// The result of cleaning is held until confirmed transfer to the contractor.
    pub fn claim_cleanup(&self, id: OperationId) -> Option<CleanupClaim<'_>> {
        self.cleanup.claim(id)
    }

    /// Records a negative outcome in the memory of the current launch.
    pub fn execute_record(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
    ) -> Result<(), EffectOwnerError> {
        self.execute_record_with(registry, id, effect, false)
    }

    /// It accepts only the already confirmed, purified positive outcome.
    /// This method does not confirm delivery and does not create the right of the recipient.
    #[allow(dead_code)]
    pub(crate) fn execute_delivered_record(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
    ) -> Result<(), EffectOwnerError> {
        self.execute_record_with(registry, id, effect, true)
    }

    fn execute_record_with(
        &self,
        registry: &OperationRegistry,
        id: OperationId,
        effect: PlannedEffect,
        delivered: bool,
    ) -> Result<(), EffectOwnerError> {
        if effect.spec != EffectSpec::RecordTerminal {
            return Err(EffectOwnerError::NotPending);
        }
        let claim = self.claim_effect(id, effect)?;
        {
            let mut records = self
                .records
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if records.contains_key(&id) {
                return Err(EffectOwnerError::Duplicate);
            }
            if records.len() >= BATCHES_MAX {
                return Err(EffectOwnerError::Limit);
            }
            records.insert(id, RecordSlot::Running);
        }
        let result = if delivered {
            registry.record_delivered_verification(id, effect)
        } else {
            registry.record_negative_verification(id, effect)
        };
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(id, RecordSlot::Ready(RecordResult::Completed(result)));
        claim.acknowledge();
        Ok(())
    }

    /// Takes a recorded step only after preserving its effects.
    /// Errors and interruptions remain with the owner until recovery.
    pub fn accept_record(&self, id: OperationId) -> Result<(), EffectOwnerError> {
        let claim = self.claim_record(id).ok_or(EffectOwnerError::NotPending)?;
        let RecordResult::Completed(Ok(step)) = claim.result() else {
            return Err(EffectOwnerError::UnsupportedRecord);
        };
        self.accept_step(id, step)?;
        claim.acknowledge();
        Ok(())
    }

    /// If the holder is lost, the result of the record is returned to a limited box.
    pub fn claim_record(&self, id: OperationId) -> Option<RecordClaim<'_>> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let slot = records.get_mut(&id)?;
        if !matches!(slot, RecordSlot::Ready(_)) {
            return None;
        }
        let RecordSlot::Ready(result) = std::mem::replace(slot, RecordSlot::Claimed) else {
            unreachable!()
        };
        Some(RecordClaim {
            owner: self,
            id,
            result: Some(result),
        })
    }
}

#[cfg(test)]
#[path = "owner.tests.rs"]
mod publication_tests;
