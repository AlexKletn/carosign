use super::owner_delivery::check_owner_verification;
use super::{
    AdmissionFailure, CleanupFailure, CleanupProgress, CleanupStage, ConnectionHandle,
    ConnectionUsage, DEFAULT_CONNECTIONS, DeferredControlFailure, Entry, IdempotentAdmission,
    MutationBindingFailure, OPERATIONS_MAX, OPERATIONS_PER_OWNER, ObservationFailure,
    ObservationProgress, OperationGuard, OwnedOperation, OwnerDeliveryFailure,
    OwnerVerificationDelivery, PreparedOwnerVerification, RegistryEntries,
    ScheduledCleanupProgress, TerminalRecordFailure, TerminalRecordStage, TrustedCallerBinding,
    VerificationBindingFailure, VerificationBindingStage, VerificationDispatchStage,
    VerificationPreparationFailure, VerificationSend,
};
use carosign_core::authorization::ValidatedGrant;
use carosign_core::ids::{IdempotencyKey, OperationId};
use carosign_runtime::OperationActor;
use carosign_runtime::ports::CallerContext;
use std::collections::BTreeSet;
use std::sync::Mutex;

fn cancel_unsent_positive_reservation(entry: &mut Entry) {
    if let Some(reservation) = entry.positive_reservation.as_ref() {
        let mut hub = entry.events.lock().unwrap_or_else(|e| e.into_inner());
        if hub.cancel_reserved_terminal(reservation).is_ok() {
            entry.positive_reservation = None;
        }
    }
}

fn same_verification_frame_context(
    a: &carosign_adapters::provider::VerificationFrameContext,
    b: &carosign_adapters::provider::VerificationFrameContext,
) -> bool {
    a.operation == b.operation
        && a.worker == b.worker
        && a.command == b.command
        && a.authenticator == b.authenticator
        && a.provider_subject == b.provider_subject
        && a.uid == b.uid
        && a.purpose == b.purpose
        && a.challenge == b.challenge
        && a.recipient == b.recipient
}

pub struct OperationRegistry {
    pub(super) entries: Mutex<RegistryEntries>,
    pub(super) evidence: crate::evidence_slot::EvidenceStore,
}

impl Default for OperationRegistry {
    fn default() -> Self {
        Self {
            entries: Mutex::new(RegistryEntries::default()),
            evidence: crate::evidence_slot::EvidenceStore::new(OPERATIONS_MAX)
                .expect("ненулевой предел таблицы операций"),
        }
    }
}

impl OperationRegistry {
    /// It binds the altering operation to a live connection, and stable idempotence is
    /// checked by the log owner before sending the command.
    pub(crate) fn reserve_mutation(
        &self,
        mut actor: OperationActor,
        grant: ValidatedGrant,
        caller: &CallerContext,
        connection: ConnectionHandle,
    ) -> Result<OperationId, AdmissionFailure> {
        let context = &actor.state().common.context;
        if actor.state().common.phase != carosign_engine::OperationPhase::New
            || !matches!(
                actor.state().body,
                carosign_engine::OperationBody::Enrollment { .. }
                    | carosign_engine::OperationBody::Removal { .. }
            )
            || actor.has_work()
            || grant.scope() != &actor.state().required_grant_scope()
            || grant.id() != context.owner.binding
            || grant.issuer() != context.owner.authority
            || grant.boot() != context.deadline.boot()
        {
            return Err(AdmissionFailure::Duplicate);
        }
        let subject = context.subject;
        let operation = context.operation_id;
        let deadline = context.deadline;
        let owner = *caller.transaction().as_bytes();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let usage = entries
            .connections
            .get(&connection)
            .ok_or(AdmissionFailure::Limit)?;
        if usage.detached || usage.operations.len() >= usage.limit {
            return Err(AdmissionFailure::Limit);
        }
        if entries.len() >= OPERATIONS_MAX
            || entries
                .values()
                .filter(|entry| entry.owner == owner && !entry.terminal)
                .count()
                >= OPERATIONS_PER_OWNER
        {
            return Err(AdmissionFailure::Limit);
        }
        if entries.contains_key(&operation) || entries.retired.contains_key(&operation) {
            return Err(AdmissionFailure::Duplicate);
        }
        crate::serve::admit(&mut actor).map_err(|_| AdmissionFailure::Duplicate)?;
        let events = std::sync::Arc::new(Mutex::new(
            carosign_runtime::operation::hub::EventHub::new(operation),
        ));
        let owned = OwnedOperation {
            actor,
            grant,
            deferred_control_step: None,
            owner: caller.clone(),
            delivery_reserved: false,
            events: events.clone(),
            terminal_disconnects: Vec::new(),
            lease: None,
            lease_matches_request: false,
            cleanup_stage: CleanupStage::NotStarted,
            terminal_record_stage: TerminalRecordStage::NotStarted,
        };
        entries.insert(
            operation,
            Entry {
                cancellation: None,
                is_mutation: true,
                subject,
                owner,
                owner_connection: Some(connection),
                operation: Some(owned),
                deadline,
                pending_control: Vec::new(),
                events,
                delivery_invalidated: false,
                terminal: false,
                verification_dispatch: VerificationDispatchStage::NotStarted,
                verification_dispatch_step: None,
                verification_binding: VerificationBindingStage::NotStarted,
                retained_delivery: None,
                positive_reservation: None,
                matched_evidence_step: None,
                matched_enrollment: None,
                evidence_evaluation_step: None,
                owner_revalidation_step: None,
                evidence_destroy_step: None,
                mutation_binding_step: None,
            },
        );
        entries
            .connections
            .get_mut(&connection)
            .expect("живое соединение")
            .operations
            .insert(operation);
        Ok(operation)
    }
}

impl OperationRegistry {
    /// Verifies the ownership of the record to the same owner who will be responsible for its
    /// recall and destruction at the end of the operation.
    pub(crate) fn owns_scoped_assertion(
        &self,
        operation: OperationId,
        assessment: &carosign_runtime::ports::ScopedAssertionAssessment,
    ) -> bool {
        self.evidence
            .reference_for(operation, assessment.evidence())
            .is_ok_and(|reference| &reference == assessment.reference())
    }

    /// Holds the transition to verification only to record its own storage.
    /// Evaluation and its issuance remain with the caller until the next stage.
    pub(crate) fn stage_scoped_assertion(
        &self,
        operation: OperationId,
        assessment: &carosign_runtime::ports::ScopedAssertionAssessment,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<(), ObservationFailure> {
        if !self.owns_scoped_assertion(operation, assessment) {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut guard = self.checkout(operation)?;
        let owned = guard.operation_mut();
        if owned.grant.boot() != now.boot()
            || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated
            || !entry.pending_control.is_empty()
            || entry.matched_evidence_step.is_some()
        {
            return Err(ObservationFailure::AwaitingActor);
        }
        let step = owned
            .actor
            .finish_scoped_assertion_transfer(assessment, now)
            .map_err(ObservationFailure::Rejected)?;
        if !matches!(
            step,
            carosign_runtime::operation::actor::Step::Advanced { .. }
        ) {
            return Err(ObservationFailure::ContextMismatch);
        }
        entry.matched_evidence_step = Some((step, assessment.reference().clone()));
        Ok(())
    }

    /// Accepts an already verified estimate of a limited record, the issue remains withheld
    /// for the transition; a change in management cannot bypass it.
    pub(crate) fn apply_scoped_assertion(
        &self,
        operation: OperationId,
        effect: carosign_engine::PlannedEffect,
        assessment: &carosign_runtime::ports::ScopedAssertionAssessment,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase};
        use carosign_runtime::OperationMsg;

        if effect.spec != EffectSpec::VerifyEvidence
            || !assessment.still_valid(now)
            || !self.owns_scoped_assertion(operation, assessment)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut guard = self.checkout(operation)?;
        let owned = guard.operation_mut();
        if owned.grant.boot() != now.boot()
            || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
            || owned.actor.state().common.phase != OperationPhase::EvaluatingEvidence
            || owned
                .actor
                .state()
                .common
                .context
                .deadline
                .is_expired_at(now)
            || owned.actor.state().common.cancel_requested
            || owned.actor.has_work()
            || owned.actor.has_control_work()
            || !owned.actor.pending().contains(&effect)
            || owned
                .actor
                .state()
                .body
                .verification()
                .and_then(|v| v.evidence)
                .is_none_or(|e| !e.belongs_to(operation) || e.id() != assessment.evidence())
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated
            || !entry.pending_control.is_empty()
            || entry.evidence_evaluation_step.is_some()
        {
            return Err(ObservationFailure::AwaitingActor);
        }
        owned
            .actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::EvidenceAccepted,
            }))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        let step = owned.actor.run_once();
        if !matches!(
            step,
            carosign_runtime::operation::actor::Step::Advanced { .. }
        ) {
            return Err(ObservationFailure::ContextMismatch);
        }
        entry.evidence_evaluation_step = Some((effect, step));
        Ok(())
    }

    pub(crate) fn assertion_store(&self) -> &crate::evidence_slot::EvidenceStore {
        &self.evidence
    }
    #[cfg(test)]
    pub(crate) fn fixture_record_sent_verification(
        &self,
        context: carosign_adapters::provider::VerificationFrameContext,
    ) {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries.get_mut(&context.operation).unwrap();
        entry.verification_binding = VerificationBindingStage::HandedOff { context };
        entry.verification_dispatch = VerificationDispatchStage::Sent;
    }

    /// Verifies that the launch of supervision relates to the accepted operation of the
    /// owner.
    /// This does not confirm the origin of the channel or the context of the provider.
    pub(crate) fn check_verification_flow_owner(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        caller: &CallerContext,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{OperationBody, OperationPhase};
        let mut guard = self.checkout(context.operation)?;
        if guard.delivery_invalidated() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        if !matches!(state.body, OperationBody::Verification(_))
            || state.common.phase != OperationPhase::New
            || state.common.context.operation_id != context.operation
            || state.common.context.subject != context.provider_subject
            || state.common.context.authenticator != context.authenticator
            || state.common.context.purpose != context.purpose
            || !owned.belongs_to(caller)
            || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
            || owned.actor.has_control_work()
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if state.common.context.deadline.is_expired_at(now)
            || owned.grant.boot() != now.boot()
            || owned.grant.deadline_ns() <= now.instant_ns()
        {
            return Err(ObservationFailure::DeadlineExpired);
        }
        Ok(())
    }

    /// Verifies the planned armament with the saved actor and issue.
    /// It is only a test of context: the source of authority, the live issue and the stable
    /// journal are obliged to check the composition that the arm will perform.
    #[allow(dead_code)]
    pub(crate) fn check_mutation_arm_context(
        &self,
        request: &carosign_runtime::ports::MutationArmRequest,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase};
        let mut guard = self.checkout(request.operation)?;
        if guard.delivery_invalidated() {
            return Err(ObservationFailure::AwaitingActor);
        }
        {
            let entries = self
                .entries
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let entry = entries
                .get(&request.operation)
                .ok_or(ObservationFailure::UnknownOperation)?;
            if entry.mutation_binding_step.is_some() || !entry.pending_control.is_empty() {
                return Err(ObservationFailure::AwaitingActor);
            }
        }
        let owned = guard.operation_mut();
        let actor = &owned.actor;
        let state = actor.state();
        if state.common.context.deadline.is_expired_at(now)
            || now.boot() != owned.grant.boot()
            || now.instant_ns() >= owned.grant.deadline_ns()
        {
            return Err(ObservationFailure::DeadlineExpired);
        }
        if actor.has_work() || actor.has_control_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let lease = owned
            .lease
            .as_ref()
            .ok_or(ObservationFailure::ContextMismatch)?;
        let owner = lease.owner();
        let mutation = state
            .mutation()
            .ok_or(ObservationFailure::ContextMismatch)?;
        if !request.is_consistent()
            || !request.matches_operation_label(state)
            || state.common.phase != OperationPhase::Arming
            || state.common.cancel_requested
            || !owned.lease_matches_request
            || !crate::run::grant_matches_operation(actor, &owned.grant)
            || state.common.context.operation_id != request.operation
            || state.common.context.subject != request.logical_plan.subject()
            || state.common.context.authenticator.id != request.logical_plan.authenticator()
            || mutation.mutation != request.arm.mutation
            || mutation.plan.as_ref() != Some(&request.logical_plan)
            || lease.lease_id() != request.lease
            || lease.resources().is_empty()
            || owner.operation != request.operation
            || owner.authenticator != state.common.context.authenticator
            || owner.worker != request.arm.worker
            || owner.authenticator.device_epoch != request.arm.epoch
            || state.common.lease != Some(request.lease)
            || state.common.worker.is_none_or(|worker| {
                worker.instance != request.arm.worker || worker.device_epoch != request.arm.epoch
            })
            || state.common.command.is_none_or(|command| {
                command.id != request.arm.command
                    || command.transition_index != request.effect.transition
            })
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut pending = actor
            .pending()
            .iter()
            .filter(|effect| effect.spec == EffectSpec::PersistDispatchArmed);
        let arm = *pending.next().ok_or(ObservationFailure::ContextMismatch)?;
        if pending.next().is_some() {
            return Err(ObservationFailure::ContextMismatch);
        }
        // It's a pure prediction; the receipt is not sent to the actor, nor is it evidence of
        // fixation.
        let decision = carosign_engine::decide(
            state,
            DomainInput::Receipt {
                effect: arm,
                receipt: EffectReceipt::DispatchArmedDurable,
            },
        )
        .map_err(|_| ObservationFailure::ContextMismatch)?;
        if decision.next.common.phase != OperationPhase::InFlight
            || decision.effects.as_slice() != [request.effect]
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        Ok(())
    }

    /// Reverts authority and live issuance to the actor's binding, and the sources selected
    /// by the cast are separately responsible for their trusted origins.
    /// The step is stored in the table until the owner accepts the effects.
    #[allow(dead_code)]
    pub(crate) async fn bind_mutation_dispatch(
        &self,
        request: &carosign_runtime::ports::MutationArmRequest,
        clock: &dyn carosign_runtime::ports::TimeSource,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        leases: &dyn crate::resources::MutationLeaseVerifier,
    ) -> Result<(), MutationBindingFailure> {
        use carosign_core::authorization::AuthorizationBoundary;
        use carosign_runtime::OperationMsg;
        let (scope, grant, caller) = {
            let mut guard = self
                .checkout(request.operation)
                .map_err(MutationBindingFailure::Operation)?;
            if guard.delivery_invalidated() {
                return Err(MutationBindingFailure::Operation(
                    ObservationFailure::AwaitingActor,
                ));
            }
            let owned = guard.operation_mut();
            let (scope, grant, _) = mutation_binding_snapshot(owned, request, clock.now())
                .map_err(MutationBindingFailure::Operation)?;
            (scope, grant, owned.owner.clone())
        };
        if authority
            .subject_uid(request.logical_plan.subject())
            .map_err(MutationBindingFailure::Authority)?
            != request.uid
        {
            return Err(MutationBindingFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        authority
            .revalidate_caller(&caller, &grant, AuthorizationBoundary::BeforeDispatch)
            .await
            .map_err(MutationBindingFailure::Authority)?;
        leases
            .validate(&scope)
            .await
            .map_err(MutationBindingFailure::Resource)?;
        authority
            .revalidate_caller(&caller, &grant, AuthorizationBoundary::BeforeDispatch)
            .await
            .map_err(MutationBindingFailure::Authority)?;

        let mut guard = self
            .checkout(request.operation)
            .map_err(MutationBindingFailure::Operation)?;
        let (current_scope, current_grant, input) =
            mutation_binding_snapshot(guard.operation_mut(), request, clock.now())
                .map_err(MutationBindingFailure::Operation)?;
        if guard.operation_mut().owner != caller
            || current_scope != scope
            || current_grant.id() != grant.id()
            || current_grant.issuer() != grant.issuer()
            || current_grant.scope() != grant.scope()
            || current_grant.boot() != grant.boot()
            || current_grant.deadline_ns() != grant.deadline_ns()
        {
            return Err(MutationBindingFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry =
            entries
                .get_mut(&request.operation)
                .ok_or(MutationBindingFailure::Operation(
                    ObservationFailure::UnknownOperation,
                ))?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(MutationBindingFailure::Operation(
                ObservationFailure::AwaitingActor,
            ));
        }
        if entry.mutation_binding_step.is_some() {
            return Err(MutationBindingFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        let actor = &mut guard.operation_mut().actor;
        actor
            .post(OperationMsg::Domain(input))
            .map_err(|_| MutationBindingFailure::Operation(ObservationFailure::MailboxFull))?;
        let step = actor.run_once();
        entry.mutation_binding_step = Some(step);
        Ok(())
    }

    /// Transmits the saved step; the owner's failure does not lose it.
    #[allow(dead_code)]
    pub(crate) fn handoff_mutation_binding(
        &self,
        operation: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), crate::effect_owner::EffectOwnerError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(crate::effect_owner::EffectOwnerError::NotPending);
        }
        let step = entry
            .mutation_binding_step
            .as_ref()
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        effects.accept_step(operation, step)?;
        entry.mutation_binding_step = None;
        Ok(())
    }

    /// An empty table with time constraints.
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes a limited statement from the retained actor.
    /// The correlation of the session does not confirm the origin or right of entry; the
    /// independent auditor is obliged to establish them separately.
    #[allow(dead_code)]
    pub(crate) fn take_observed_scoped_assertion(
        &self,
        operation: OperationId,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<carosign_runtime::provider::session::ScopedProviderAssertion, ObservationFailure>
    {
        let mut guard = self.checkout(operation)?;
        let owned = guard.operation_mut();
        if owned
            .actor
            .state()
            .common
            .context
            .deadline
            .is_expired_at(now)
            || owned.grant.boot() != now.boot()
            || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        owned
            .actor
            .take_scoped_provider_assertion()
            .map_err(ObservationFailure::Rejected)
    }

    /// Accepts opaque material from the actor's observation already withheld.
    /// The token is moved without copying, and the given step is held until acceptance by the
    /// performer.
    #[allow(dead_code)]
    pub(crate) fn hold_observed_verification_token(
        &self,
        operation: OperationId,
        id: carosign_core::ids::EvidenceId,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<carosign_evidence::material::EvidenceMaterialRef, ObservationFailure> {
        use carosign_engine::OperationPhase;
        let mut guard = self.checkout(operation)?;
        let actor = &mut guard.operation_mut().actor;
        let state = actor.state();
        if state.common.phase != OperationPhase::Running
            || state.common.cancel_requested
            || actor.has_work()
            || actor.has_control_work()
            || state
                .body
                .verification()
                .is_none_or(|v| v.evidence.is_some())
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let deadline = state.common.context.deadline;
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated
            || !entry.pending_control.is_empty()
            || entry.matched_evidence_step.is_some()
        {
            return Err(ObservationFailure::AwaitingActor);
        }
        let observation = actor
            .verification_observation()
            .ok_or(ObservationFailure::ContextMismatch)?;
        let carosign_provider_sdk::events::Event::Verification {
            matched_enrollment: Some(wire),
            ..
        } = observation.event()
        else {
            return Err(ObservationFailure::ContextMismatch);
        };
        let enrollment = observation
            .permitted()
            .bindings()
            .iter()
            .find(|item| {
                item.enrollment.id.as_bytes().as_slice() == wire.enrollment_id.as_slice()
                    && item.enrollment.authenticator.as_bytes().as_slice()
                        == wire.authenticator_id.as_slice()
                    && item.enrollment.generation.value() == wire.generation
            })
            .map(|item| item.enrollment)
            .ok_or(ObservationFailure::ContextMismatch)?;
        let token = actor
            .take_matched_token()
            .map_err(ObservationFailure::Rejected)?;
        let material = self
            .evidence
            .hold(id, operation, deadline, token, now)
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        let step = match actor.finish_matched_transfer(&material) {
            Ok(step @ carosign_runtime::operation::actor::Step::Advanced { .. }) => step,
            _ => {
                let _ = self.evidence.invalidate(material.clone());
                return Err(ObservationFailure::ContextMismatch);
            }
        };
        entry.matched_evidence_step = Some((step, material.clone()));
        entry.matched_enrollment = Some(enrollment);
        Ok(material)
    }

    /// Keeps the transition until the owner accepts the effects.
    #[allow(dead_code)]
    pub(crate) fn handoff_matched_evidence(
        &self,
        operation: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), crate::effect_owner::EffectOwnerError> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(crate::effect_owner::EffectOwnerError::UnsupportedControl);
        }
        let (step, _) = entry
            .matched_evidence_step
            .as_ref()
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        effects.accept_step(operation, step)?;
        entry.matched_evidence_step = None;
        Ok(())
    }

    /// Collects the evaluation area from the saved operation, sending and response.
    /// This does not confirm the origin of the channel, call or checker.
    #[allow(dead_code)]
    pub(crate) fn matched_evidence_context(
        &self,
        operation: OperationId,
        material: &carosign_evidence::material::EvidenceMaterialRef,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<carosign_runtime::ports::EvidenceContext, ObservationFailure> {
        use carosign_engine::{EffectSpec, OperationPhase};
        let mut guard = self.checkout(operation)?;
        let owned = guard.operation_mut();
        let actor = &owned.actor;
        let state = actor.state();
        let evidence = state
            .body
            .verification()
            .and_then(|v| v.evidence)
            .ok_or(ObservationFailure::ContextMismatch)?;
        let lease = owned
            .lease
            .as_ref()
            .ok_or(ObservationFailure::ContextMismatch)?;
        let permitted = actor
            .verification_scope()
            .ok_or(ObservationFailure::ContextMismatch)?;
        if state.common.phase != OperationPhase::EvaluatingEvidence
            || state.common.cancel_requested
            || actor.has_work()
            || actor.has_control_work()
            || !actor
                .pending()
                .iter()
                .any(|effect| effect.spec == EffectSpec::VerifyEvidence)
            || evidence.operation() != operation
            || material.operation() != *operation.as_bytes()
            || material.id() != *evidence.id().as_bytes()
            || self
                .evidence
                .reference_for(operation, evidence.id())
                .ok()
                .as_ref()
                != Some(material)
            || !crate::run::grant_matches_operation(actor, &owned.grant)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if state.common.context.deadline.is_expired_at(now)
            || owned.grant.boot() != now.boot()
            || owned.grant.deadline_ns() <= now.instant_ns()
        {
            return Err(ObservationFailure::DeadlineExpired);
        }
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let VerificationBindingStage::HandedOff { context } = &entry.verification_binding else {
            return Err(ObservationFailure::ContextMismatch);
        };
        let enrollment = entry
            .matched_enrollment
            .ok_or(ObservationFailure::ContextMismatch)?;
        let owner = lease.owner();
        if entry.verification_dispatch != VerificationDispatchStage::Sent
            || context.operation != operation
            || context.authenticator != state.common.context.authenticator
            || context.purpose != state.common.context.purpose
            || owner.operation != operation
            || owner.authenticator != context.authenticator
            || owner.worker != context.worker
            || state.common.lease != Some(lease.lease_id())
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker
                    || worker.device_epoch != context.authenticator.device_epoch
            })
            || state
                .common
                .command
                .is_none_or(|command| command.id != context.command)
            || !permitted
                .bindings()
                .iter()
                .any(|item| item.enrollment == enrollment)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        Ok(carosign_runtime::ports::EvidenceContext {
            operation,
            subject: state.common.context.subject,
            authenticator: context.authenticator,
            enrollment,
            worker: context.worker,
            permitted: permitted.clone(),
            purpose: context.purpose,
            recipient: context.recipient,
            challenge: context.challenge,
            deadline: state.common.context.deadline,
        })
    }

    /// Accepts the result of an independent auditor after reconciling.
    /// The transition is held until the effects are transferred to the owner.
    #[allow(dead_code)]
    pub(crate) fn apply_evidence_evaluation(
        &self,
        effect: carosign_engine::PlannedEffect,
        material: &carosign_evidence::material::EvidenceMaterialRef,
        expected: &carosign_runtime::ports::EvidenceContext,
        accepted: bool,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
        use carosign_runtime::{OperationMsg, operation::actor::Step};
        let current = self.matched_evidence_context(expected.operation, material, now)?;
        if effect.spec != EffectSpec::VerifyEvidence
            || current.operation != expected.operation
            || current.subject != expected.subject
            || current.authenticator != expected.authenticator
            || current.enrollment != expected.enrollment
            || current.worker != expected.worker
            || current.permitted != expected.permitted
            || current.purpose != expected.purpose
            || current.recipient != expected.recipient
            || current.challenge != expected.challenge
            || current.deadline != expected.deadline
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let mut guard = self.checkout(expected.operation)?;
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&expected.operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        if entry.evidence_evaluation_step.is_some() {
            return Err(ObservationFailure::ContextMismatch);
        }
        let actor = &mut guard.operation_mut().actor;
        if actor.has_work() || actor.has_control_work() || !actor.pending().contains(&effect) {
            return Err(ObservationFailure::AwaitingActor);
        }
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: if accepted {
                    EffectReceipt::EvidenceAccepted
                } else {
                    EffectReceipt::EvidenceRejected
                },
            }))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        let step = actor.run_once();
        if !matches!(step, Step::Advanced { .. }) {
            return Err(ObservationFailure::ContextMismatch);
        }
        entry.evidence_evaluation_step = Some((effect, step));
        Ok(())
    }

    /// Passes the transition without loss while the performer is busy.
    #[allow(dead_code)]
    pub(crate) fn handoff_evidence_evaluation(
        &self,
        operation: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), crate::effect_owner::EffectOwnerError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(crate::effect_owner::EffectOwnerError::UnsupportedControl);
        }
        let (_, step) = entry
            .evidence_evaluation_step
            .as_ref()
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        effects.accept_step(operation, step)?;
        entry.evidence_evaluation_step = None;
        Ok(())
    }

    /// Rechecks the original resolution after evaluation and holds the transition.
    /// This is not the right to deliver the result: after cleaning, the check is repeated.
    #[allow(dead_code)]
    pub(crate) async fn revalidate_evaluated_owner(
        &self,
        operation: OperationId,
        effect: carosign_engine::PlannedEffect,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), ObservationFailure> {
        self.revalidate_owner_at(operation, effect, authority, clock, false)
            .await
    }

    /// Re-checking before delivery is possible only after confirmed cleaning.
    pub(crate) async fn revalidate_cleaned_owner(
        &self,
        operation: OperationId,
        effect: carosign_engine::PlannedEffect,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<(), ObservationFailure> {
        self.revalidate_owner_at(operation, effect, authority, clock, true)
            .await
    }

    async fn revalidate_owner_at(
        &self,
        operation: OperationId,
        effect: carosign_engine::PlannedEffect,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
        after_cleanup: bool,
    ) -> Result<(), ObservationFailure> {
        use carosign_core::authorization::AuthorizationBoundary;
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase};
        use carosign_runtime::{OperationMsg, operation::actor::Step};
        let check = |owned: &OwnedOperation,
                     now: carosign_core::operation::context::BootInstant| {
            let state = owned.actor.state();
            state.common.context.operation_id == operation
                && if after_cleanup {
                    state.common.phase == OperationPhase::Finalizing
                        && state.common.terminal
                            == Some(carosign_engine::TerminalOutcome::FactorAccepted)
                        && state.common.lease.is_none()
                        && owned.lease.is_none()
                        && owned.cleanup_stage == CleanupStage::Released
                } else {
                    state.common.phase == OperationPhase::EvaluatingEvidence
                }
                && !state.common.cancel_requested
                && effect.spec == EffectSpec::RevalidateOwner
                && owned.actor.pending().contains(&effect)
                && !owned.actor.has_work()
                && !owned.actor.has_control_work()
                && crate::run::grant_matches_operation(&owned.actor, &owned.grant)
                && !state.common.context.deadline.is_expired_at(now)
                && owned.grant.boot() == now.boot()
                && now.instant_ns() < owned.grant.deadline_ns()
        };
        let (grant, caller) = {
            let mut guard = self.checkout(operation)?;
            if guard.delivery_invalidated() || !check(guard.operation_mut(), clock.now()) {
                return Err(ObservationFailure::ContextMismatch);
            }
            let owned = guard.operation_mut();
            (owned.grant.clone(), owned.owner.clone())
        };
        authority
            .revalidate_caller(&caller, &grant, AuthorizationBoundary::BeforeDelivery)
            .await
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        let now = clock.now();
        let mut guard = self.checkout(operation)?;
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated
            || !entry.pending_control.is_empty()
            || entry.owner_revalidation_step.is_some()
        {
            return Err(ObservationFailure::AwaitingActor);
        }
        let owned = guard.operation_mut();
        if !check(owned, now)
            || !owned.belongs_to(&caller)
            || owned.grant.id() != grant.id()
            || owned.grant.issuer() != grant.issuer()
            || owned.grant.scope() != grant.scope()
            || owned.grant.boot() != grant.boot()
            || owned.grant.deadline_ns() != grant.deadline_ns()
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        owned
            .actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::OwnerValid,
            }))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        let step = owned.actor.run_once();
        if !matches!(step, Step::Advanced { .. }) {
            return Err(ObservationFailure::ContextMismatch);
        }
        entry.owner_revalidation_step = Some((effect, step));
        Ok(())
    }

    /// Save the transition if the performer is busy or the recall was ahead of the transfer.
    #[allow(dead_code)]
    pub(crate) fn handoff_owner_revalidation(
        &self,
        operation: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), crate::effect_owner::EffectOwnerError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(crate::effect_owner::EffectOwnerError::UnsupportedControl);
        }
        let (_, step) = entry
            .owner_revalidation_step
            .as_ref()
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        effects.accept_step(operation, step)?;
        entry.owner_revalidation_step = None;
        Ok(())
    }

    /// Accepts the material before the reference is transmitted to the actor, the origin of
    /// the bytes, match and score are still checked by the individual composition.
    #[allow(dead_code)]
    pub(crate) fn hold_actor_evidence(
        &self,
        operation: OperationId,
        id: carosign_core::ids::EvidenceId,
        bytes: zeroize::Zeroizing<Vec<u8>>,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<carosign_evidence::material::EvidenceMaterialRef, ObservationFailure> {
        use carosign_engine::OperationPhase;
        let mut guard = self.checkout(operation)?;
        let actor = &mut guard.operation_mut().actor;
        let state = actor.state();
        if state.common.phase != OperationPhase::Running
            || state.common.cancel_requested
            || actor.has_work()
            || actor.has_control_work()
            || actor.verification_observation().is_some()
            || state
                .body
                .verification()
                .is_none_or(|v| v.evidence.is_some())
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        self.evidence
            .hold(id, operation, state.common.context.deadline, bytes, now)
            .map_err(|_| ObservationFailure::ContextMismatch)
    }

    /// It only transfers material to the actor who is waiting for it.
    /// The caller verifies the origin of the session and the evaluation separately.
    #[allow(dead_code)]
    pub(crate) fn take_actor_evidence(
        &self,
        operation: OperationId,
        material: carosign_evidence::material::EvidenceMaterialRef,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<crate::evidence_slot::EvidenceLease, ObservationFailure> {
        use carosign_engine::{EffectSpec, OperationPhase};
        let mut guard = self.checkout(operation)?;
        let actor = &mut guard.operation_mut().actor;
        let state = actor.state();
        let reference = state
            .body
            .verification()
            .and_then(|v| v.evidence)
            .ok_or(ObservationFailure::ContextMismatch)?;
        if state.common.phase != OperationPhase::EvaluatingEvidence
            || state.common.cancel_requested
            || actor.has_control_work()
            || actor.verification_observation().is_some()
            || !actor
                .pending()
                .iter()
                .any(|effect| effect.spec == EffectSpec::VerifyEvidence)
            || material.operation() != *operation.as_bytes()
            || material.id() != *reference.id().as_bytes()
            || !reference.belongs_to(operation)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        self.evidence
            .take(material, now)
            .map_err(|_| ObservationFailure::ContextMismatch)
    }

    /// It cleans the retained material, puts a receipt to the actor, and stores the
    /// transition until the effects are handed over to the owner, and a busy verifier does
    /// not confirm the destruction and requires re-delivery of the effect.
    #[allow(dead_code)]
    pub(crate) fn destroy_actor_evidence(
        &self,
        operation: OperationId,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
        use carosign_runtime::OperationMsg;
        {
            let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
            let entry = entries
                .get(&operation)
                .ok_or(ObservationFailure::UnknownOperation)?;
            if let Some((recorded, _, _)) = &entry.evidence_destroy_step {
                return if *recorded == effect {
                    Ok(())
                } else {
                    Err(ObservationFailure::ContextMismatch)
                };
            }
        }
        let mut guard = self.checkout(operation)?;
        let actor = &mut guard.operation_mut().actor;
        let reference = actor
            .state()
            .body
            .verification()
            .and_then(|v| v.evidence)
            .filter(|reference| reference.belongs_to(operation))
            .ok_or(ObservationFailure::ContextMismatch)?;
        if effect.spec != EffectSpec::DestroyEvidence
            || !actor.pending().contains(&effect)
            || actor.verification_observation().is_some()
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if actor.has_work() || actor.has_control_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let material = self
            .evidence
            .reference_for(operation, reference.id())
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        self.evidence
            .invalidate(material.clone())
            .map_err(|error| match error {
                crate::evidence_slot::SlotError::InUse => ObservationFailure::Busy,
                _ => ObservationFailure::ContextMismatch,
            })?;
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if !entry.pending_control.is_empty() || entry.evidence_destroy_step.is_some() {
            return Err(ObservationFailure::AwaitingActor);
        }
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::EvidenceDestroyed,
            }))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        let step = actor.run_once();
        if !matches!(
            step,
            carosign_runtime::operation::actor::Step::Advanced { .. }
        ) {
            return Err(ObservationFailure::ContextMismatch);
        }
        entry.evidence_destroy_step = Some((effect, step, material));
        Ok(())
    }

    /// Retransmission does not perform destruction again.
    #[allow(dead_code)]
    pub(crate) fn handoff_evidence_destroy(
        &self,
        operation: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), crate::effect_owner::EffectOwnerError> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        let (_, step, material) = entry
            .evidence_destroy_step
            .as_ref()
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        effects.accept_step(operation, step)?;
        self.evidence
            .release_after_handoff(material)
            .map_err(|_| crate::effect_owner::EffectOwnerError::UnsupportedRecord)?;
        entry.evidence_destroy_step = None;
        Ok(())
    }

    /// Compound accounting is not an authorization for operations.
    #[allow(dead_code)]
    pub(crate) fn open_connection(&self, limit: u8) -> Option<ConnectionHandle> {
        if !(1..=4).contains(&limit) {
            return None;
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if entries.connections.len() >= DEFAULT_CONNECTIONS {
            return None;
        }
        let serial = entries.next_connection.checked_add(1)?;
        entries.next_connection = serial;
        let connection = ConnectionHandle(serial);
        entries.connections.insert(
            connection,
            ConnectionUsage {
                limit: usize::from(limit),
                operations: BTreeSet::new(),
                detached: false,
            },
        );
        Some(connection)
    }

    /// A snapshot of the owner's condition to diagnose a denial of admission.
    /// It does not issue licenses or replace atomic checks on redundancy.
    pub(crate) fn connection_is_attached(&self, connection: ConnectionHandle) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .connections
            .get(&connection)
            .is_some_and(|usage| !usage.detached)
    }

    /// Frees up accounting and keeps the original owner offline.
    /// The repeat client and the observer do not become the owners of the transaction.
    #[allow(dead_code)]
    pub(crate) fn close_connection(&self, connection: ConnectionHandle) {
        self.detach_connection(connection);
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .connections
            .remove(&connection);
    }

    /// The gap recalls the owner at once, but the place remains occupied until the end of the
    /// handler, so that a lot of hanging calls can not be bypassed.
    pub(crate) fn detach_connection(&self, connection: ConnectionHandle) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(usage) = entries.connections.get_mut(&connection) else {
            return;
        };
        if usage.detached {
            return;
        }
        usage.detached = true;
        for (id, entry) in entries.operations.iter_mut() {
            if entry.owner_connection != Some(connection) {
                continue;
            }
            entry.owner_connection = None;
            eprintln!("carosignd: владелец проверки отключился; доставка отозвана");
            entry.delivery_invalidated = true;
            cancel_unsent_positive_reservation(entry);
            entry.retained_delivery = None;
            let _ = self.evidence.invalidate_operation(*id);
            if entry.terminal {
                continue;
            }
            let message = carosign_runtime::OperationMsg::OwnerDetached;
            if let Some(owned) = entry.operation.as_mut() {
                // Overflow of control is fixed by the actor and does not become a
                // confirmation of the cancellation or release of resources.
                let _ = owned.actor.post(message);
            } else if !entry.pending_control.contains(&message) {
                entry.pending_control.push(message);
            }
        }
    }

    /// Publishes the actor and record the key under one lock.
    /// Before calling, the source must verify the authority and eligibility to receive the
    /// old operation. Until the trusted binding is available, the method fails.
    #[allow(dead_code, clippy::too_many_arguments)]
    pub(crate) fn reserve_idempotent_verification(
        &self,
        binding: Option<&TrustedCallerBinding>,
        key: IdempotencyKey,
        intent: &carosign_runtime::authority::verification::VerificationIntent,
        mut actor: OperationActor,
        grant: ValidatedGrant,
        caller: &CallerContext,
        capacity: usize,
        connection: ConnectionHandle,
    ) -> IdempotentAdmission {
        let Some(binding) = binding else {
            return IdempotentAdmission::Unavailable;
        };
        let context = &actor.state().common.context;
        if actor.state().common.phase != carosign_engine::OperationPhase::New
            || !matches!(
                actor.state().body,
                carosign_engine::OperationBody::Verification(_)
            )
            || actor.has_work()
            || grant.scope() != &actor.state().required_grant_scope()
            || grant.id() != context.owner.binding
            || grant.issuer() != context.owner.authority
            || grant.boot() != context.deadline.boot()
        {
            return IdempotentAdmission::Unavailable;
        }
        let Ok(digest) = crate::verification_idempotency::RequestDigest::new(intent, grant.scope())
        else {
            return IdempotentAdmission::Unavailable;
        };
        let subject = context.subject;
        let operation = actor.state().common.context.operation_id;
        let deadline = actor.state().common.context.deadline;
        let owner = *caller.transaction().as_bytes();
        let index_key = (binding.0, key);
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if entries
            .connections
            .get(&connection)
            .is_none_or(|usage| usage.detached)
        {
            return IdempotentAdmission::Unavailable;
        }
        if let Some((old_digest, old_operation)) = entries.idempotency.get(&index_key).copied() {
            return if old_digest != digest {
                IdempotentAdmission::Conflict
            } else if let Some(entry) = entries.operations.get(&old_operation) {
                let terminal = entry.terminal;
                let Some(usage) = entries.connections.get_mut(&connection) else {
                    return IdempotentAdmission::Unavailable;
                };
                if !terminal && !usage.operations.contains(&old_operation) {
                    if usage.operations.len() >= usage.limit {
                        return IdempotentAdmission::Limit;
                    }
                    usage.operations.insert(old_operation);
                }
                IdempotentAdmission::Existing(old_operation)
            } else if entries.retired.contains_key(&old_operation) {
                IdempotentAdmission::Existing(old_operation)
            } else {
                IdempotentAdmission::Unavailable
            };
        }
        let Some(usage) = entries.connections.get(&connection) else {
            return IdempotentAdmission::Unavailable;
        };
        if usage.operations.len() >= usage.limit {
            return IdempotentAdmission::Limit;
        }
        if entries.idempotency.len() >= capacity
            || entries.len() >= OPERATIONS_MAX
            || entries
                .values()
                .filter(|entry| entry.owner == owner && !entry.terminal)
                .count()
                >= OPERATIONS_PER_OWNER
        {
            return IdempotentAdmission::Limit;
        }
        if entries.contains_key(&operation)
            || entries.retired.contains_key(&operation)
            || entries
                .idempotency
                .values()
                .any(|(_, old)| *old == operation)
        {
            return IdempotentAdmission::Unavailable;
        }
        // Only a new operation is allowed: the key repeat is returned higher and does not put
        // a second message to an already published actor.
        if crate::serve::admit(&mut actor).is_err() {
            return IdempotentAdmission::Unavailable;
        }
        let events = std::sync::Arc::new(Mutex::new(
            carosign_runtime::operation::hub::EventHub::new(operation),
        ));
        let owned = OwnedOperation {
            actor,
            grant,
            deferred_control_step: None,
            owner: caller.clone(),
            delivery_reserved: false,
            events: std::sync::Arc::clone(&events),
            terminal_disconnects: Vec::new(),
            lease: None,
            lease_matches_request: false,
            cleanup_stage: CleanupStage::NotStarted,
            terminal_record_stage: TerminalRecordStage::NotStarted,
        };
        entries.insert(
            operation,
            Entry {
                cancellation: None,
                is_mutation: false,
                subject,
                owner,
                owner_connection: Some(connection),
                operation: Some(owned),
                deadline,
                pending_control: Vec::new(),
                events,
                delivery_invalidated: false,
                terminal: false,
                verification_dispatch: VerificationDispatchStage::NotStarted,
                verification_dispatch_step: None,
                verification_binding: VerificationBindingStage::NotStarted,
                retained_delivery: None,
                positive_reservation: None,
                matched_evidence_step: None,
                matched_enrollment: None,
                evidence_evaluation_step: None,
                owner_revalidation_step: None,
                evidence_destroy_step: None,
                mutation_binding_step: None,
            },
        );
        entries.idempotency.insert(index_key, (digest, operation));
        entries
            .connections
            .get_mut(&connection)
            .expect("соединение под блокировкой")
            .operations
            .insert(operation);
        IdempotentAdmission::Created(operation)
    }

    pub(crate) fn reserve(
        &self,
        actor: OperationActor,
        grant: ValidatedGrant,
        caller: &CallerContext,
    ) -> Result<OperationGuard<'_>, AdmissionFailure> {
        let is_mutation = actor.state().is_mutation();
        let subject = actor.state().common.context.subject;
        let operation = actor.state().common.context.operation_id;
        let deadline = actor.state().common.context.deadline;
        let owner = *caller.transaction().as_bytes();
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if entries.contains_key(&operation)
            || entries.retired.contains_key(&operation)
            || entries
                .idempotency
                .values()
                .any(|(_, old)| *old == operation)
        {
            return Err(AdmissionFailure::Duplicate);
        }
        if entries.len() >= OPERATIONS_MAX
            || entries
                .values()
                .filter(|entry| entry.owner == owner && !entry.terminal)
                .count()
                >= OPERATIONS_PER_OWNER
        {
            return Err(AdmissionFailure::Limit);
        }
        let events = std::sync::Arc::new(Mutex::new(
            carosign_runtime::operation::hub::EventHub::new(operation),
        ));
        entries.insert(
            operation,
            Entry {
                cancellation: None,
                is_mutation,
                subject,
                owner,
                owner_connection: None,
                operation: None,
                deadline,
                pending_control: Vec::new(),
                events: std::sync::Arc::clone(&events),
                delivery_invalidated: false,
                terminal: false,
                verification_dispatch: VerificationDispatchStage::NotStarted,
                verification_dispatch_step: None,
                verification_binding: VerificationBindingStage::NotStarted,
                retained_delivery: None,
                positive_reservation: None,
                matched_evidence_step: None,
                matched_enrollment: None,
                evidence_evaluation_step: None,
                owner_revalidation_step: None,
                evidence_destroy_step: None,
                mutation_binding_step: None,
            },
        );
        Ok(OperationGuard {
            registry: self,
            id: operation,
            operation: Some(OwnedOperation {
                actor,
                grant,
                deferred_control_step: None,
                owner: caller.clone(),
                delivery_reserved: false,
                events,
                terminal_disconnects: Vec::new(),
                lease: None,
                lease_matches_request: false,
                cleanup_stage: CleanupStage::NotStarted,
                terminal_record_stage: TerminalRecordStage::NotStarted,
            }),
        })
    }

    /// Saved actor phase for internal diagnostics; not authorization.
    pub fn phase(&self, id: OperationId) -> Option<carosign_engine::OperationPhase> {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)?
            .operation
            .as_ref()
            .map(|entry| entry.actor.state().common.phase)
    }

    /// The flow belongs to the operation; reference is not a power to observe.
    pub(crate) fn event_hub(
        &self,
        id: OperationId,
    ) -> Result<std::sync::Arc<Mutex<carosign_runtime::operation::hub::EventHub>>, ObservationFailure>
    {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)
            .map(|entry| std::sync::Arc::clone(&entry.events))
            .ok_or(ObservationFailure::UnknownOperation)
    }

    pub(crate) fn publish_provider_progress(
        &self,
        observation: carosign_runtime::provider::session::ProgressObservation,
    ) -> Result<Option<carosign_runtime::operation::events::Event>, ObservationFailure> {
        self.checkout(observation.operation)?
            .publish_provider_progress(observation.progress)
    }

    /// A copy of notifications held until confirmation of closure of connections.
    #[allow(dead_code)]
    pub(crate) fn terminal_disconnects(
        &self,
        id: OperationId,
    ) -> Result<Vec<u32>, ObservationFailure> {
        let mut guard = self.checkout(id)?;
        Ok(guard.operation_mut().terminal_disconnects.clone())
    }

    /// Confirms the processing of exactly the received set of notifications.
    #[allow(dead_code)]
    pub(crate) fn acknowledge_terminal_disconnects(
        &self,
        id: OperationId,
        expected: &[u32],
    ) -> Result<(), ObservationFailure> {
        let mut guard = self.checkout(id)?;
        let notifications = &mut guard.operation_mut().terminal_disconnects;
        if notifications.as_slice() != expected {
            return Err(ObservationFailure::ContextMismatch);
        }
        notifications.clear();
        Ok(())
    }

    /// Transfers the previously delayed control to the owner of the effects, the step is
    /// first held together with the actor and does not disappear if the queue is occupied.
    pub(crate) fn transfer_deferred_control(
        &self,
        id: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), DeferredControlFailure> {
        let mut guard = self
            .checkout_for_control(id)
            .map_err(DeferredControlFailure::Operation)?;
        let owned = guard.operation_mut();
        if owned.deferred_control_step.is_none() {
            if !owned.actor.has_control_work() {
                return Err(DeferredControlFailure::NoControl);
            }
            owned.deferred_control_step = Some(owned.actor.run_once());
        }
        let step = owned
            .deferred_control_step
            .as_ref()
            .expect("шаг удерживается");
        effects
            .accept_step(id, step)
            .map_err(DeferredControlFailure::Effects)?;
        owned.deferred_control_step = None;
        Ok(())
    }

    /// It transmits a signal from a trusted observer to recall the saved operation.
    /// The caller must perform the effects of the return step.
    pub fn revoke_verification(
        &self,
        id: OperationId,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        let mut guard = self
            .checkout_or_defer_control(id, carosign_runtime::OperationMsg::GrantRevoked, None)?
            .ok_or(ObservationFailure::ContextMismatch)?;
        let actor = &mut guard.operation_mut().actor;
        actor
            .post(carosign_runtime::OperationMsg::GrantRevoked)
            .map_err(|_| ObservationFailure::MailboxFull)?;
        Ok(actor.run_once())
    }

    /// Cancels the result of the operation and holds the step for the effects owner.
    pub(crate) fn cancel_verification(
        &self,
        id: OperationId,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        let mut guard = self
            .checkout_or_defer_control(id, carosign_runtime::OperationMsg::CancelRequested, None)?
            .ok_or(ObservationFailure::ContextMismatch)?;
        let actor = &mut guard.operation_mut().actor;
        actor
            .post(carosign_runtime::OperationMsg::CancelRequested)
            .map_err(|_| ObservationFailure::MailboxFull)?;
        Ok(actor.run_once())
    }

    /// Transmits the expiration of the term by a trusted clock; someone else's download has
    /// expired.
    pub fn check_verification_deadline(
        &self,
        id: OperationId,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<Option<carosign_runtime::operation::actor::Step>, ObservationFailure> {
        let Some(mut guard) = self.checkout_or_defer_control(
            id,
            carosign_runtime::OperationMsg::DeadlineElapsed,
            Some(now),
        )?
        else {
            return Ok(None);
        };
        let actor = &mut guard.operation_mut().actor;
        if !actor.state().common.context.deadline.is_expired_at(now) {
            return Ok(None);
        }
        actor
            .post(carosign_runtime::OperationMsg::DeadlineElapsed)
            .map_err(|_| ObservationFailure::MailboxFull)?;
        Ok(Some(actor.run_once()))
    }

    fn checkout_or_defer_control(
        &self,
        id: OperationId,
        message: carosign_runtime::OperationMsg,
        now: Option<carosign_core::operation::context::BootInstant>,
    ) -> Result<Option<OperationGuard<'_>>, ObservationFailure> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&id)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if now.is_some_and(|instant| !entry.deadline.is_expired_at(instant)) {
            return Ok(None);
        }
        entry.delivery_invalidated = true;
        // The ready-made reserve hasn't started recording yet, and if it's already started or
        // unspecified, the hub refuses to cancel and holds the number.
        cancel_unsent_positive_reservation(entry);
        // The signal is not waiting for the actor or the tester: even with a busy actor,
        // bytes are recalled, and the working issue loses the right to the result.
        let _ = self.evidence.invalidate_operation(id);
        // The retracted image must not remain available for return to the owner even when the
        // actor is busy.
        entry.retained_delivery = None;
        if entry
            .operation
            .as_ref()
            .is_some_and(|operation| operation.deferred_control_step.is_some())
        {
            if !entry.pending_control.contains(&message) {
                entry.pending_control.push(message);
            }
            return Err(ObservationFailure::Busy);
        }
        match entry.operation.take() {
            Some(operation) => Ok(Some(OperationGuard {
                registry: self,
                id,
                operation: Some(operation),
            })),
            None => {
                if !entry.pending_control.contains(&message) {
                    entry.pending_control.push(message);
                }
                Err(ObservationFailure::Busy)
            }
        }
    }

    /// Whether the output is stored in the memory of the process; it is not a confirmation of
    /// peace.
    pub fn has_retained_lease(&self, id: OperationId) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)
            .and_then(|entry| entry.operation.as_ref())
            .is_some_and(|operation| operation.lease.is_some())
    }

    /// Prepares a negative final outcome for the original owner.
    ///
    /// The source of the policy is called without a global lock. Status and deadline are
    /// re-checked after `await`The picture is not a confirmation of the shipment, and the
    /// access revoked after the return requires a watcher.
    pub async fn prepare_owner_verification(
        &self,
        id: OperationId,
        caller: &CallerContext,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<PreparedOwnerVerification, OwnerDeliveryFailure> {
        let (grant, outcome) = {
            let mut guard = self.checkout(id).map_err(OwnerDeliveryFailure::Operation)?;
            if guard.delivery_invalidated() {
                return Err(OwnerDeliveryFailure::Authority(
                    carosign_runtime::ports::AuthorityRefusal::Revoked,
                ));
            }
            let owned = guard.operation_mut();
            let outcome = check_owner_verification(owned, caller, clock.now(), false)?;
            (owned.grant.clone(), outcome)
        };
        authority
            .revalidate_caller(
                caller,
                &grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
            .map_err(OwnerDeliveryFailure::Authority)?;
        let mut guard = self.checkout(id).map_err(OwnerDeliveryFailure::Operation)?;
        if guard.delivery_invalidated() {
            return Err(OwnerDeliveryFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Revoked,
            ));
        }
        let owned = guard.operation_mut();
        let current = check_owner_verification(owned, caller, clock.now(), false)?;
        if current != outcome
            || owned.grant.id() != grant.id()
            || owned.grant.issuer() != grant.issuer()
            || owned.grant.scope() != grant.scope()
            || owned.grant.boot() != grant.boot()
            || owned.grant.deadline_ns() != grant.deadline_ns()
        {
            return Err(OwnerDeliveryFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        let sequence = guard.publish_owner_terminal()?;
        Ok(PreparedOwnerVerification {
            operation: id,
            outcome,
            sequence,
        })
    }

    /// Creates an owner at the original registry so that an unwritten image returns to the
    /// table when the holder is lost in the current launch.
    #[allow(dead_code)]
    pub(crate) async fn prepare_owner_delivery<'a>(
        &'a self,
        id: OperationId,
        caller: &CallerContext,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<OwnerVerificationDelivery<'a>, OwnerDeliveryFailure> {
        let prepared = self
            .prepare_owner_verification(id, caller, authority, clock)
            .await?;
        Ok(OwnerVerificationDelivery::new(
            self,
            prepared,
            caller.clone(),
        ))
    }

    /// Allows the original live connection to receive an already recorded cancellation.
    /// A positive delivery on cancellation is never restored.
    pub(crate) fn allow_cancelled_delivery(
        &self,
        id: OperationId,
        caller: &CallerContext,
        connection: ConnectionHandle,
    ) -> Result<(), ObservationFailure> {
        let mut entries = self.entries.lock().map_err(|_| ObservationFailure::Busy)?;
        let entry = entries
            .get_mut(&id)
            .ok_or(ObservationFailure::UnknownOperation)?;
        let owned = entry.operation.as_ref().ok_or(ObservationFailure::Busy)?;
        if entry.owner_connection != Some(connection)
            || !entry.pending_control.is_empty()
            || !owned.belongs_to(caller)
            || owned.lease.is_some()
            || owned.cleanup_stage != CleanupStage::Released
            || owned.actor.has_work()
            || owned.actor.has_control_work()
            || owned.actor.state().common.phase != carosign_engine::OperationPhase::Terminal
            || owned.actor.state().common.terminal
                != Some(carosign_engine::TerminalOutcome::Cancelled)
            || !matches!(
                owned.terminal_record_stage,
                TerminalRecordStage::Recorded {
                    outcome: carosign_engine::TerminalOutcome::Cancelled,
                    ..
                }
            )
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        entry.delivery_invalidated = false;
        Ok(())
    }

    /// Repeat only the retained, not yet sent photo.
    /// When delivering, the authority and term are checked again.
    #[allow(dead_code)]
    pub(crate) fn reclaim_owner_delivery<'a>(
        &'a self,
        id: OperationId,
        caller: &CallerContext,
    ) -> Result<OwnerVerificationDelivery<'a>, OwnerDeliveryFailure> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.get_mut(&id).ok_or(OwnerDeliveryFailure::Operation(
            ObservationFailure::UnknownOperation,
        ))?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            entry.retained_delivery = None;
            return Err(OwnerDeliveryFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Revoked,
            ));
        }
        let retained = entry
            .retained_delivery
            .as_ref()
            .ok_or(OwnerDeliveryFailure::AlreadyReserved)?;
        if &retained.caller != caller {
            return Err(OwnerDeliveryFailure::WrongOwner);
        }
        let retained = entry
            .retained_delivery
            .take()
            .expect("снимок удержан под блокировкой");
        let mut delivery = OwnerVerificationDelivery::new(self, retained.prepared, retained.caller);
        delivery.cursor = retained.cursor;
        Ok(delivery)
    }

    /// Notes the lack of confirmation of cleaning of a specific issue.
    ///
    /// This path doesn't bring up the old one. `ResourcePort::release`: he consumes the
    /// refund even if the outcome is unknown. Leasing stays at the operation, and the actor
    /// requests quarantine. The resulting quarantine effect is obliged to perform separately;
    /// recording in the actor does not protect the device.
    pub fn mark_cleanup_uncertain(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<carosign_runtime::operation::actor::Step, CleanupFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationBody};
        use carosign_runtime::OperationMsg;

        let mut guard = self
            .checkout(id)
            .map_err(|_| CleanupFailure::OperationUnavailable)?;
        let owned = guard.operation_mut();
        let actor = &mut owned.actor;
        if !matches!(actor.state().body, OperationBody::Verification(_)) {
            return Err(CleanupFailure::WrongOperationKind);
        }
        let lease = owned
            .lease
            .as_ref()
            .ok_or(CleanupFailure::NoRetainedLease)?;
        let owner = lease.owner();
        if owner.operation != id
            || owner.authenticator != actor.state().common.context.authenticator
            || actor
                .state()
                .common
                .lease
                .is_some_and(|lease_id| lease_id != lease.lease_id())
            || actor.state().common.worker.is_some_and(|worker| {
                worker.instance != owner.worker
                    || worker.device_epoch != owner.authenticator.device_epoch
            })
        {
            return Err(CleanupFailure::LeaseContextMismatch);
        }
        if effect.spec != EffectSpec::ReleaseResources || !actor.pending().contains(&effect) {
            return Err(CleanupFailure::EffectMismatch);
        }
        if actor.has_work() {
            return Err(CleanupFailure::AwaitingActor);
        }
        owned.cleanup_stage = CleanupStage::Uncertain;
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::ResourceUncertain,
            }))
            .map_err(|_| CleanupFailure::MailboxFull)?;
        Ok(actor.run_once())
    }

    /// Blocks matching withheld issuance in the local scheduler.
    /// This is a conservative measure: the effect and leasing remain with the operation, and
    /// evidence of quarantine or physical quiescence is not created.
    pub(crate) fn block_uncertain_scheduler_lease(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
        scheduler: &mut carosign_runtime::scheduler::Scheduler,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{EffectSpec, OperationBody};
        let mut guard = self.checkout(id)?;
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        let lease = owned
            .lease
            .as_ref()
            .ok_or(ObservationFailure::ContextMismatch)?;
        let owner = lease.owner();
        if !matches!(state.body, OperationBody::Verification(_))
            || effect.spec != EffectSpec::QuarantineScope
            || !owned.actor.pending().contains(&effect)
            || !owned.lease_matches_request
            || owner.operation != id
            || owner.authenticator != state.common.context.authenticator
            || state.common.lease != Some(lease.lease_id())
            || state.common.worker.is_none_or(|worker| {
                worker.instance != owner.worker
                    || worker.device_epoch != owner.authenticator.device_epoch
            })
            || lease.resources().is_empty()
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if !scheduler.mark_uncertain_scope(lease.lease_id(), owner, lease.resource_set()) {
            return Err(ObservationFailure::ContextMismatch);
        }
        Ok(())
    }

    /// It protects the exact issuance before the external waiting of the broker.
    /// The loss of the holder preserves the fencing; the staging itself does not prove peace,
    /// the origin of the leasing or a successful cleaning.
    #[allow(dead_code)]
    pub(crate) fn begin_scheduled_cleanup(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
        scheduler: &mut carosign_runtime::scheduler::Scheduler,
    ) -> Result<carosign_runtime::scheduler::Quiescing, CleanupFailure> {
        use carosign_engine::{EffectSpec, OperationBody};
        let mut guard = self
            .checkout(id)
            .map_err(|_| CleanupFailure::OperationUnavailable)?;
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        if !matches!(state.body, OperationBody::Verification(_)) {
            return Err(CleanupFailure::WrongOperationKind);
        }
        let lease = owned
            .lease
            .as_ref()
            .ok_or(CleanupFailure::NoRetainedLease)?;
        let owner = lease.owner();
        if !owned.lease_matches_request
            || owner.operation != id
            || owner.authenticator != state.common.context.authenticator
            || state.common.lease != Some(lease.lease_id())
            || state.common.worker.is_none_or(|worker| {
                worker.instance != owner.worker
                    || worker.device_epoch != owner.authenticator.device_epoch
            })
        {
            return Err(CleanupFailure::LeaseContextMismatch);
        }
        if effect.spec != EffectSpec::ReleaseResources || !owned.actor.pending().contains(&effect) {
            return Err(CleanupFailure::EffectMismatch);
        }
        if owned.actor.has_work() || owned.actor.has_control_work() {
            return Err(CleanupFailure::AwaitingActor);
        }
        if owned.cleanup_stage != CleanupStage::NotStarted {
            return Err(CleanupFailure::OperationUnavailable);
        }
        let held = scheduler
            .begin_quiescing(lease.lease_id(), owner, lease.resource_set())
            .ok_or(CleanupFailure::LeaseContextMismatch)?;
        owned.cleanup_stage = CleanupStage::Quiescing;
        Ok(held)
    }

    /// It protects the delivery until the wait, and the mediator checks the answers and
    /// trust.
    /// In case of future loss, the fencing and issue remain withheld.
    pub(crate) async fn cleanup_verification_scheduled(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn crate::resources::TrustedCleanupBroker,
    ) -> Result<ScheduledCleanupProgress, CleanupFailure> {
        self.cleanup_verification_scheduled_inner(id, effect, scheduler, broker, None)
            .await
    }

    pub(crate) async fn cleanup_verification_scheduled_at(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        clock: &dyn carosign_runtime::ports::TimeSource,
    ) -> Result<ScheduledCleanupProgress, CleanupFailure> {
        self.cleanup_verification_scheduled_inner(id, effect, scheduler, broker, Some(clock))
            .await
    }

    async fn cleanup_verification_scheduled_inner(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
        scheduler: &Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        clock: Option<&dyn carosign_runtime::ports::TimeSource>,
    ) -> Result<ScheduledCleanupProgress, CleanupFailure> {
        use carosign_engine::{EffectReceipt, EffectSpec, OperationBody};
        use carosign_runtime::scheduler::cleanup::{confirm_quiesced_for, confirm_released};
        let mut guard = self
            .checkout(id)
            .map_err(|_| CleanupFailure::OperationUnavailable)?;
        let dispatch = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .map(|entry| entry.verification_dispatch)
            .ok_or(CleanupFailure::OperationUnavailable)?;
        let dispatch_uncertain = matches!(
            dispatch,
            VerificationDispatchStage::Sending | VerificationDispatchStage::OutcomeUnknown
        );
        let (scope, held) = {
            let owned = guard.operation_mut();
            let state = owned.actor.state();
            if !matches!(state.body, OperationBody::Verification(_)) {
                return Err(CleanupFailure::WrongOperationKind);
            }
            let lease = owned
                .lease
                .as_ref()
                .ok_or(CleanupFailure::NoRetainedLease)?;
            let owner = lease.owner();
            let bound = state.common.lease == Some(lease.lease_id())
                && state.common.worker.is_some_and(|worker| {
                    worker.instance == owner.worker
                        && worker.device_epoch == owner.authenticator.device_epoch
                });
            let before_binding = dispatch == VerificationDispatchStage::NotStarted
                && state.common.lease.is_none()
                && state.common.worker.is_none();
            if !owned.lease_matches_request
                || owner.operation != id
                || owner.authenticator != state.common.context.authenticator
                || !(bound || before_binding)
            {
                return Err(CleanupFailure::LeaseContextMismatch);
            }
            if effect.spec != EffectSpec::ReleaseResources
                || !owned.actor.pending().contains(&effect)
            {
                return Err(CleanupFailure::EffectMismatch);
            }
            if owned.actor.has_work() || owned.actor.has_control_work() {
                return Err(CleanupFailure::AwaitingActor);
            }
            if owned.cleanup_stage != CleanupStage::NotStarted {
                return Err(CleanupFailure::OperationUnavailable);
            }
            let scope = crate::resources::CleanupScope::of(lease);
            let held = scheduler
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .begin_quiescing(lease.lease_id(), owner, lease.resource_set())
                .ok_or(CleanupFailure::LeaseContextMismatch)?;
            owned.cleanup_stage = CleanupStage::Quiescing;
            (scope, held)
        };
        // The owner of the cleaning gets the opportunity to check the timer after the fencing
        // and before the first contact with the broker.
        let mut fenced = false;
        std::future::poll_fn(|cx| {
            if fenced {
                std::task::Poll::Ready(())
            } else {
                fenced = true;
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        })
        .await;
        if (!dispatch_uncertain || broker.processes_stopped(&scope))
            && let Ok(proof) = confirm_quiesced_for(broker, &scope, &held).await
        {
            guard.operation_mut().cleanup_stage = CleanupStage::Releasing;
            if let Ok(released) = confirm_released(broker, &scope, proof).await {
                let mut scheduler_guard = scheduler.lock().unwrap_or_else(|e| e.into_inner());
                let granted = match clock {
                    Some(clock) => scheduler_guard.finish_quiescing_at(held, released, clock.now()),
                    None => scheduler_guard.finish_quiescing(held, released),
                };
                drop(scheduler_guard);
                match granted {
                    Ok(()) => {
                        guard.operation_mut().cleanup_stage = CleanupStage::Released;
                        guard.operation_mut().lease.take();
                        let step =
                            guard.post_cleanup_receipt(effect, EffectReceipt::CleanupConfirmed)?;
                        return Ok(ScheduledCleanupProgress {
                            progress: CleanupProgress::Confirmed(step),
                        });
                    }
                    Err(held) => {
                        scheduler
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .quiescing_uncertain(held);
                        guard.operation_mut().cleanup_stage = CleanupStage::Uncertain;
                        let step =
                            guard.post_cleanup_receipt(effect, EffectReceipt::ResourceUncertain)?;
                        return Ok(ScheduledCleanupProgress {
                            progress: CleanupProgress::Uncertain(step),
                        });
                    }
                }
            }
        }
        scheduler
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .quiescing_uncertain(held);
        guard.operation_mut().cleanup_stage = CleanupStage::Uncertain;
        let step = guard.post_cleanup_receipt(effect, EffectReceipt::ResourceUncertain)?;
        Ok(ScheduledCleanupProgress {
            progress: CleanupProgress::Uncertain(step),
        })
    }

    /// Consistently requests peace and release from a trusted mediator.
    ///
    /// The output remains with the operation when the response fails, the response
    /// mismatches, and the future is interrupted, and the total blockage of the table during
    /// calls is not held.
    /// The mediator is obliged to verify the current era and confirm physical peace; the test
    /// implementation is not hardware evidence.
    /// The caller must limit the duration of both stages; the interruption of the future
    /// keeps the output in a state of uncertainty.
    #[cfg(test)]
    pub async fn cleanup_verification(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
        broker: &dyn crate::resources::TrustedCleanupBroker,
    ) -> Result<CleanupProgress, CleanupFailure> {
        use carosign_engine::{EffectReceipt, EffectSpec, OperationBody};
        let mut guard = self
            .checkout(id)
            .map_err(|_| CleanupFailure::OperationUnavailable)?;
        // Until the shipment is completed or its outcome is unknown, even the instantaneous
        // response of the broker does not prove that the command will not reach the
        // worker after the release of the issue.
        let dispatch_uncertain = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)
            .is_none_or(|entry| {
                matches!(
                    entry.verification_dispatch,
                    VerificationDispatchStage::Sending | VerificationDispatchStage::OutcomeUnknown
                )
            });
        let scope = {
            let owned = guard.operation_mut();
            if !matches!(owned.actor.state().body, OperationBody::Verification(_)) {
                return Err(CleanupFailure::WrongOperationKind);
            }
            let lease = owned
                .lease
                .as_ref()
                .ok_or(CleanupFailure::NoRetainedLease)?;
            let owner = lease.owner();
            if owner.operation != id
                || owner.authenticator != owned.actor.state().common.context.authenticator
                || owned
                    .actor
                    .state()
                    .common
                    .lease
                    .is_some_and(|current| current != lease.lease_id())
                || owned.actor.state().common.worker.is_some_and(|worker| {
                    worker.instance != owner.worker
                        || worker.device_epoch != owner.authenticator.device_epoch
                })
            {
                return Err(CleanupFailure::LeaseContextMismatch);
            }
            if effect.spec != EffectSpec::ReleaseResources
                || !owned.actor.pending().contains(&effect)
            {
                return Err(CleanupFailure::EffectMismatch);
            }
            if owned.actor.has_work() {
                return Err(CleanupFailure::AwaitingActor);
            }
            crate::resources::CleanupScope::of(lease)
        };
        if (dispatch_uncertain || !guard.operation_mut().lease_matches_request)
            && guard.operation_mut().cleanup_stage == CleanupStage::NotStarted
        {
            // The broker may have given out an incomplete or someone else’s set; it is
            // not supposed to be released, but the uncertainty must remain.
            guard.operation_mut().cleanup_stage = CleanupStage::Uncertain;
        }
        match guard.operation_mut().cleanup_stage {
            CleanupStage::NotStarted => {
                guard.operation_mut().cleanup_stage = CleanupStage::Quiescing;
                let quiesced =
                    carosign_runtime::scheduler::cleanup::confirm_quiesced(broker, &scope).await;
                if let Ok(proof) = quiesced {
                    guard.operation_mut().cleanup_stage = CleanupStage::Releasing;
                    let released = carosign_runtime::scheduler::cleanup::confirm_released(
                        broker, &scope, proof,
                    )
                    .await;
                    if released.is_ok() {
                        guard.operation_mut().cleanup_stage = CleanupStage::Released;
                        guard.operation_mut().lease.take();
                        let step =
                            guard.post_cleanup_receipt(effect, EffectReceipt::CleanupConfirmed)?;
                        return Ok(CleanupProgress::Confirmed(step));
                    }
                }
                guard.operation_mut().cleanup_stage = CleanupStage::Uncertain;
            }
            CleanupStage::Uncertain => {}
            CleanupStage::Released => return Err(CleanupFailure::AlreadyFinished),
            CleanupStage::Quiescing | CleanupStage::Releasing => {
                return Err(CleanupFailure::OperationUnavailable);
            }
        }
        let step = guard.post_cleanup_receipt(effect, EffectReceipt::ResourceUncertain)?;
        Ok(CleanupProgress::Uncertain(step))
    }

    /// Reserves the result number and holds the reserve in the transaction table.
    /// This is not a check of the origin of the recipient or permission to send.
    #[allow(dead_code)]
    pub(crate) fn reserve_positive_delivery(
        &self,
        id: OperationId,
        caller: &CallerContext,
        effect: carosign_engine::PlannedEffect,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<u64, OwnerDeliveryFailure> {
        use carosign_engine::{EffectSpec, OperationPhase, TerminalOutcome};
        let mut guard = self.checkout(id).map_err(OwnerDeliveryFailure::Operation)?;
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        if !owned.belongs_to(caller) {
            return Err(OwnerDeliveryFailure::WrongOwner);
        }
        if state.common.phase != OperationPhase::Finalizing
            || state.common.terminal != Some(TerminalOutcome::FactorAccepted)
            || state.common.cancel_requested
            || owned.lease.is_some()
            || state.common.lease.is_some()
            || owned.cleanup_stage != CleanupStage::Released
            || owned.actor.has_work()
            || owned.actor.has_control_work()
            || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
            || effect.spec != EffectSpec::DeliverOutcome
            || !owned.actor.pending().contains(&effect)
        {
            return Err(OwnerDeliveryFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        if state.common.context.deadline.is_expired_at(now)
            || owned.grant.boot() != now.boot()
            || owned.grant.deadline_ns() <= now.instant_ns()
        {
            return Err(OwnerDeliveryFailure::Operation(
                ObservationFailure::DeadlineExpired,
            ));
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.get_mut(&id).ok_or(OwnerDeliveryFailure::Operation(
            ObservationFailure::UnknownOperation,
        ))?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(OwnerDeliveryFailure::Authority(
                carosign_runtime::ports::AuthorityRefusal::Revoked,
            ));
        }
        if entry.positive_reservation.is_some() {
            return Err(OwnerDeliveryFailure::AlreadyReserved);
        }
        let mut hub = entry.events.lock().unwrap_or_else(|e| e.into_inner());
        let reservation = hub
            .reserve_positive_terminal(state, effect)
            .map_err(|_| OwnerDeliveryFailure::Operation(ObservationFailure::ContextMismatch))?;
        let sequence = reservation.sequence();
        drop(hub);
        entry.positive_reservation = Some(reservation);
        Ok(sequence)
    }

    /// Fixes a negative outcome in the limited owner of this launch.
    /// The record does not get to the disk and after restarting it is not restored.
    pub(crate) fn record_negative_verification(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<carosign_runtime::operation::actor::Step, TerminalRecordFailure> {
        use carosign_engine::{EffectSpec, OperationBody, OperationPhase, TerminalOutcome};
        let mut guard = self
            .checkout(id)
            .map_err(|_| TerminalRecordFailure::OperationUnavailable)?;
        let owned = guard.operation_mut();
        if owned.terminal_record_stage != TerminalRecordStage::NotStarted {
            return Err(TerminalRecordFailure::AlreadyRecorded);
        }
        let state = owned.actor.state();
        if !matches!(state.body, OperationBody::Verification(_))
            || state.common.phase != OperationPhase::Finalizing
            || !matches!(
                state.common.terminal,
                Some(
                    TerminalOutcome::NoSample
                        | TerminalOutcome::NotMatched
                        | TerminalOutcome::Cancelled
                        | TerminalOutcome::Unavailable
                        | TerminalOutcome::Failed
                )
            )
            || owned.lease.is_some()
            || state.common.lease.is_some()
            || owned.cleanup_stage != CleanupStage::Released
            || effect.spec != EffectSpec::RecordTerminal
            || !owned.actor.pending().contains(&effect)
        {
            return Err(TerminalRecordFailure::ContextMismatch);
        }
        if owned.actor.has_work() {
            return Err(TerminalRecordFailure::AwaitingActor);
        }
        guard.record_verification_terminal(effect)
    }

    /// It only records a positive outcome that has already been delivered and cleared in the
    /// memory of the current launch, and the method itself does not confirm the socket entry.
    #[allow(dead_code)]
    pub(crate) fn record_delivered_verification(
        &self,
        id: OperationId,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<carosign_runtime::operation::actor::Step, TerminalRecordFailure> {
        use carosign_engine::transition::VerificationDelivery;
        use carosign_engine::{EffectSpec, OperationBody, OperationPhase, TerminalOutcome};
        let mut guard = self
            .checkout(id)
            .map_err(|_| TerminalRecordFailure::OperationUnavailable)?;
        let owned = guard.operation_mut();
        if owned.terminal_record_stage != TerminalRecordStage::NotStarted {
            return Err(TerminalRecordFailure::AlreadyRecorded);
        }
        let state = owned.actor.state();
        if !matches!(&state.body, OperationBody::Verification(verification)
            if verification.evidence.is_none() && verification.delivery == VerificationDelivery::Sent)
            || state.common.phase != OperationPhase::Finalizing
            || state.common.terminal != Some(TerminalOutcome::FactorAccepted)
            || owned.lease.is_some()
            || state.common.lease.is_some()
            || owned.cleanup_stage != CleanupStage::Released
            || effect.spec != EffectSpec::RecordTerminal
            || !owned.actor.pending().contains(&effect)
        {
            return Err(TerminalRecordFailure::ContextMismatch);
        }
        if owned.actor.has_work() || owned.actor.has_control_work() {
            return Err(TerminalRecordFailure::AwaitingActor);
        }
        guard.record_verification_terminal(effect)
    }

    /// Publishes a retained number only after a positive outcome is recorded and a confirmed
    /// delivery is confirmed. Returned shutdowns must be processed by the owner of the event
    /// stream.
    #[allow(dead_code)]
    pub(crate) fn publish_delivered_verification(
        &self,
        id: OperationId,
    ) -> Result<Vec<u32>, OwnerDeliveryFailure> {
        use carosign_engine::TerminalOutcome;
        let mut guard = self.checkout(id).map_err(OwnerDeliveryFailure::Operation)?;
        let owned = guard.operation_mut();
        if !matches!(
            owned.terminal_record_stage,
            TerminalRecordStage::Recorded {
                outcome: TerminalOutcome::FactorAccepted,
                ..
            }
        ) || owned.cleanup_stage != CleanupStage::Released
            || owned.lease.is_some()
            || owned.actor.has_work()
            || owned.actor.has_control_work()
        {
            return Err(OwnerDeliveryFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.get_mut(&id).ok_or(OwnerDeliveryFailure::Operation(
            ObservationFailure::UnknownOperation,
        ))?;
        let reservation =
            entry
                .positive_reservation
                .as_ref()
                .ok_or(OwnerDeliveryFailure::Operation(
                    ObservationFailure::ContextMismatch,
                ))?;
        let mut hub = entry.events.lock().unwrap_or_else(|e| e.into_inner());
        let disconnected = hub
            .publish_reserved_terminal(reservation, owned.actor.state())
            .map_err(|_| OwnerDeliveryFailure::Operation(ObservationFailure::ContextMismatch))?;
        drop(hub);
        entry.positive_reservation = None;
        Ok(disconnected)
    }

    /// Stores the binding step until the confirmed transfer to the owner of the effects.
    pub(crate) fn retain_verification_binding(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let entry = entries
                .get_mut(&context.operation)
                .ok_or(ObservationFailure::UnknownOperation)?;
            if entry.delivery_invalidated || !entry.pending_control.is_empty() {
                return Err(ObservationFailure::AwaitingActor);
            }
            if !matches!(
                entry.verification_binding,
                VerificationBindingStage::NotStarted
            ) {
                return Err(ObservationFailure::ContextMismatch);
            }
            entry.verification_binding = VerificationBindingStage::Binding;
        }
        let result = self.prepare_verification_dispatch(context.operation, context.command);
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&context.operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        match result {
            Ok(step @ carosign_runtime::operation::actor::Step::Advanced { .. }) => {
                entry.verification_binding = VerificationBindingStage::Ready {
                    context: context.clone(),
                    step: step.clone(),
                };
                Ok(step)
            }
            Ok(_) => {
                entry.verification_binding = VerificationBindingStage::NotStarted;
                Err(ObservationFailure::ContextMismatch)
            }
            Err(error) => {
                entry.verification_binding = VerificationBindingStage::NotStarted;
                Err(error)
            }
        }
    }

    pub(crate) fn retained_verification_binding(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(&context.operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        match &entry.verification_binding {
            VerificationBindingStage::Ready {
                context: saved,
                step,
            } if same_verification_frame_context(saved, context) => Ok(step.clone()),
            _ => Err(ObservationFailure::ContextMismatch),
        }
    }

    pub(crate) fn handoff_verification_binding(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), VerificationBindingFailure> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry =
            entries
                .get_mut(&context.operation)
                .ok_or(VerificationBindingFailure::Operation(
                    ObservationFailure::UnknownOperation,
                ))?;
        if entry.delivery_invalidated || !entry.pending_control.is_empty() {
            return Err(VerificationBindingFailure::Operation(
                ObservationFailure::AwaitingActor,
            ));
        }
        let VerificationBindingStage::Ready {
            context: saved,
            step,
        } = &entry.verification_binding
        else {
            return Err(VerificationBindingFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        };
        if !same_verification_frame_context(saved, context) {
            return Err(VerificationBindingFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        effects
            .accept_step(context.operation, step)
            .map_err(VerificationBindingFailure::Handoff)?;
        entry.verification_binding = VerificationBindingStage::HandedOff {
            context: context.clone(),
        };
        Ok(())
    }

    /// Assigns the prepared shipment to the owner of the issued resources.
    ///
    /// The session owner is required to verify the origin of the issuance, authority and
    /// deadline before sending, and this method does not send a command or prove the
    /// worker's trusted start-up or the physical safety of the device.
    pub fn prepare_verification_dispatch(
        &self,
        operation: OperationId,
        command: carosign_core::ids::CommandId,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        use carosign_engine::{DomainInput, EffectSpec, WorkerRef};
        use carosign_runtime::OperationMsg;
        let mut guard = self.checkout(operation)?;
        let owned = guard.operation_mut();
        let lease = owned
            .lease
            .as_ref()
            .ok_or(ObservationFailure::ContextMismatch)?;
        let owner = lease.owner();
        let actor = &mut owned.actor;
        if actor.state().common.context.operation_id != owner.operation
            || actor.state().common.context.authenticator != owner.authenticator
            || !crate::run::grant_matches_operation(actor, &owned.grant)
            || !owned.lease_matches_request
            || lease.resources().is_empty()
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if actor.has_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let effect = *actor
            .pending()
            .iter()
            .find(|effect| effect.spec == EffectSpec::SendVerify)
            .ok_or(ObservationFailure::ContextMismatch)?;
        actor
            .post(OperationMsg::Domain(
                DomainInput::BindVerificationDispatch {
                    effect,
                    lease: lease.lease_id(),
                    worker: WorkerRef {
                        instance: owner.worker,
                        device_epoch: owner.authenticator.device_epoch,
                    },
                    command,
                },
            ))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        Ok(actor.run_once())
    }

    /// Tie the command to the withheld rendition and recheck its frame.
    /// The step returns even when the policy is abandoned: the caller must pass it on to the
    /// owner of the effects, and must not lose the transition already made.
    /// The method itself does not send anything and does not confirm trust in the channel.
    #[allow(dead_code)]
    pub(crate) async fn bind_and_prepare_verification(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        clock: &dyn carosign_runtime::ports::TimeSource,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
    ) -> Result<
        (
            carosign_runtime::operation::actor::Step,
            Result<
                (
                    carosign_protocol::carosign::provider::v1::ProviderFrame,
                    carosign_runtime::PermittedSet,
                ),
                VerificationPreparationFailure,
            >,
        ),
        ObservationFailure,
    > {
        let step = self.prepare_verification_dispatch(context.operation, context.command)?;
        let prepared = if matches!(
            &step,
            carosign_runtime::operation::actor::Step::Advanced { .. }
        ) {
            self.revalidated_verification_frame(context, clock, authority)
                .await
        } else {
            Err(VerificationPreparationFailure::Operation(
                ObservationFailure::ContextMismatch,
            ))
        };
        Ok((step, prepared))
    }

    /// Collects a frame from a fixed set and verified context.
    ///
    /// The construction does not allow sending: origin `provider_subject`, call and receiver,
    /// validity of authority and trusted channel shall be separately checked by the session
    /// owner immediately before it.
    pub(crate) fn verification_frame(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        now: carosign_core::operation::context::BootInstant,
    ) -> Result<
        (
            carosign_protocol::carosign::provider::v1::ProviderFrame,
            carosign_runtime::PermittedSet,
        ),
        ObservationFailure,
    > {
        use carosign_engine::{EffectSpec, OperationPhase};
        let mut guard = self.checkout(context.operation)?;
        let owned = guard.operation_mut();
        let actor = &owned.actor;
        let state = actor.state();
        let lease = owned
            .lease
            .as_ref()
            .ok_or(ObservationFailure::ContextMismatch)?;
        let owner = lease.owner();
        let pending = actor
            .pending()
            .iter()
            .find(|effect| effect.spec == EffectSpec::SendVerify)
            .ok_or(ObservationFailure::ContextMismatch)?;
        if state.common.context.deadline.is_expired_at(now) {
            return Err(ObservationFailure::DeadlineExpired);
        }
        if actor.has_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        if state.common.phase != OperationPhase::Running
            || state.common.cancel_requested
            || !owned.lease_matches_request
            || !crate::run::grant_matches_operation(actor, &owned.grant)
            || state.common.context.operation_id != context.operation
            || state.common.context.subject != context.provider_subject
            || state.common.context.authenticator != context.authenticator
            || state.common.context.purpose != context.purpose
            || owner.operation != context.operation
            || owner.authenticator != context.authenticator
            || owner.worker != context.worker
            || state.common.lease != Some(lease.lease_id())
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker
                    || worker.device_epoch != context.authenticator.device_epoch
            })
            || state.common.command.is_none_or(|command| {
                command.id != context.command || command.transition_index != pending.transition
            })
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let permitted = actor
            .verification_scope()
            .ok_or(ObservationFailure::ContextMismatch)?
            .clone();
        let frame = carosign_adapters::provider::verification_frame(context, &permitted)
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        Ok((frame, permitted))
    }

    /// Re-check the resolution and context before sending the message to the session.
    ///
    /// Between preparation and sending, the session owner is obliged to monitor the recall
    /// and the deadline; the returned message is not in itself an authority and does not
    /// confirm the source of the call or a trusted channel.
    pub async fn revalidated_verification_frame(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        clock: &dyn carosign_runtime::ports::TimeSource,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
    ) -> Result<
        (
            carosign_protocol::carosign::provider::v1::ProviderFrame,
            carosign_runtime::PermittedSet,
        ),
        VerificationPreparationFailure,
    > {
        use carosign_core::authorization::AuthorizationBoundary;
        self.verification_frame(context, clock.now())
            .map_err(VerificationPreparationFailure::Operation)?;
        let (grant, caller) = self
            .verification_authority_snapshot(context.operation)
            .map_err(VerificationPreparationFailure::Operation)?;
        if authority
            .subject_uid(context.provider_subject)
            .map_err(VerificationPreparationFailure::Authority)?
            != context.uid
        {
            return Err(VerificationPreparationFailure::Operation(
                ObservationFailure::ContextMismatch,
            ));
        }
        authority
            .revalidate_caller(&caller, &grant, AuthorizationBoundary::BeforeDispatch)
            .await
            .map_err(VerificationPreparationFailure::Authority)?;
        let prepared = self
            .verification_frame(context, clock.now())
            .map_err(VerificationPreparationFailure::Operation)?;
        let (current, current_caller) = self
            .verification_authority_snapshot(context.operation)
            .map_err(VerificationPreparationFailure::Operation)?;
        if caller != current_caller
            || grant.id() != current.id()
            || grant.issuer() != current.issuer()
            || grant.scope() != current.scope()
            || grant.boot() != current.boot()
            || grant.deadline_ns() != current.deadline_ns()
        {
            return Err(VerificationPreparationFailure::GrantChanged);
        }
        Ok(prepared)
    }

    fn verification_authority_snapshot(
        &self,
        operation: OperationId,
    ) -> Result<(ValidatedGrant, CallerContext), ObservationFailure> {
        let mut guard = self.checkout(operation)?;
        let owned = guard.operation_mut();
        if !crate::run::grant_matches_operation(&owned.actor, &owned.grant) {
            return Err(ObservationFailure::ContextMismatch);
        }
        Ok((owned.grant.clone(), owned.owner.clone()))
    }

    /// Transmits the result previously verified by the provider's session.
    ///
    /// The session is read before the call. The function does not verify the recipient's
    /// authority, does not perform effects, and does not confirm the cleaning or the result
    /// of the factor.
    pub fn apply_verification_observation(
        &self,
        id: OperationId,
        observation: carosign_runtime::provider::session::VerificationObservation,
    ) -> Result<
        ObservationProgress,
        (
            ObservationFailure,
            Option<Box<carosign_runtime::provider::session::VerificationObservation>>,
        ),
    > {
        let mut guard = match self.checkout(id) {
            Ok(guard) => guard,
            Err(error) => return Err((error, Some(Box::new(observation)))),
        };
        let actor = &mut guard.operation_mut().actor;
        actor
            .hold_verification_observation(observation)
            .map_err(|error| (ObservationFailure::Rejected(error), None))?;
        match actor.advance_verification_observation() {
            Ok(()) => Ok(ObservationProgress::Advanced(actor.run_once())),
            Err(error) => Ok(ObservationProgress::Held(error)),
        }
    }

    /// Rechecks the authority and UID until a single attempt is secured.
    /// Oversight of the recall and trust in the channel remain with the staff.
    pub(crate) async fn begin_revalidated_verification_send<'a>(
        &'a self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        clock: &dyn carosign_runtime::ports::TimeSource,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
    ) -> Result<VerificationSend<'a>, VerificationPreparationFailure> {
        self.revalidated_verification_frame(context, clock, authority)
            .await?;
        self.begin_verification_send(context, clock.now(), authority)
            .map_err(VerificationPreparationFailure::Operation)
    }

    fn begin_verification_send(
        &self,
        context: &carosign_adapters::provider::VerificationFrameContext,
        now: carosign_core::operation::context::BootInstant,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
    ) -> Result<VerificationSend<'_>, ObservationFailure> {
        use carosign_engine::{EffectSpec, OperationPhase};
        if authority.subject_uid(context.provider_subject).ok() != Some(context.uid) {
            return Err(ObservationFailure::ContextMismatch);
        }
        let (frame, permitted) = self.verification_frame(context, now)?;
        let mut guard = self.checkout(context.operation)?;
        let owned = guard.operation_mut();
        let actor = &owned.actor;
        let state = actor.state();
        let effect = *actor
            .pending()
            .iter()
            .find(|effect| effect.spec == EffectSpec::SendVerify)
            .ok_or(ObservationFailure::ContextMismatch)?;
        let lease = owned
            .lease
            .as_ref()
            .ok_or(ObservationFailure::ContextMismatch)?;
        if actor.has_work() || actor.has_control_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        if state.common.phase != OperationPhase::Running
            || state.common.cancel_requested
            || state.common.context.authenticator != context.authenticator
            || state.common.context.purpose != context.purpose
            || state.common.context.deadline.is_expired_at(now)
            || !owned.lease_matches_request
            || !crate::run::grant_matches_operation(actor, &owned.grant)
            || lease.owner().operation != context.operation
            || lease.owner().worker != context.worker
            || lease.owner().authenticator != context.authenticator
            || state.common.lease != Some(lease.lease_id())
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker
                    || worker.device_epoch != context.authenticator.device_epoch
            })
            || state.common.command.is_none_or(|command| {
                command.id != context.command || command.transition_index != effect.transition
            })
            || actor.verification_scope() != Some(&permitted)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let entry = entries
                .get_mut(&context.operation)
                .ok_or(ObservationFailure::UnknownOperation)?;
            if !entry.pending_control.is_empty() || entry.delivery_invalidated {
                return Err(ObservationFailure::AwaitingActor);
            }
            if entry.verification_dispatch != VerificationDispatchStage::NotStarted {
                return Err(ObservationFailure::ContextMismatch);
            }
            entry.verification_dispatch = VerificationDispatchStage::Sending;
        }
        Ok(VerificationSend {
            registry: self,
            operation: context.operation,
            frame: Some(frame),
            permitted: Some(permitted),
            finished: false,
        })
    }

    /// Delivers a saved outcome; a busy actor does not bypass.
    #[allow(dead_code)]
    pub(crate) fn deliver_verification_send_failure(
        &self,
        context: &crate::provider_verification::VerificationContext,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase};
        use carosign_runtime::OperationMsg;
        let mut guard = self.checkout(context.operation)?;
        let owned = guard.operation_mut();
        let actor = &mut owned.actor;
        let state = actor.state();
        if state.common.context.authenticator.id != context.authenticator
            || state.common.context.authenticator.device_epoch != context.epoch
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker || worker.device_epoch != context.epoch
            })
            || state
                .common
                .command
                .is_none_or(|command| command.id != context.command)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if actor.has_work() || actor.has_control_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&context.operation)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if !entry.pending_control.is_empty() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let (effect, receipt, cleanup_uncertain) =
            match (state.common.phase, entry.verification_dispatch) {
                (
                    OperationPhase::Running,
                    stage @ (VerificationDispatchStage::NotSent
                    | VerificationDispatchStage::OutcomeUnknown),
                ) => {
                    let effect = *actor
                        .pending()
                        .iter()
                        .find(|effect| effect.spec == EffectSpec::SendVerify)
                        .ok_or(ObservationFailure::ContextMismatch)?;
                    if state
                        .common
                        .command
                        .is_none_or(|command| command.transition_index != effect.transition)
                    {
                        return Err(ObservationFailure::ContextMismatch);
                    }
                    let receipt = if stage == VerificationDispatchStage::NotSent {
                        EffectReceipt::DispatchNotSent
                    } else {
                        EffectReceipt::DispatchOutcomeUnknown
                    };
                    (effect, receipt, false)
                }
                (OperationPhase::Finalizing, VerificationDispatchStage::OutcomeUnknown)
                    if entry.delivery_invalidated && state.common.cancel_requested =>
                {
                    let lease = owned
                        .lease
                        .as_ref()
                        .ok_or(ObservationFailure::ContextMismatch)?;
                    if lease.owner().operation != context.operation
                        || lease.owner().worker != context.worker
                        || lease.owner().authenticator != state.common.context.authenticator
                        || state.common.lease != Some(lease.lease_id())
                        || !matches!(
                            owned.cleanup_stage,
                            CleanupStage::NotStarted | CleanupStage::Uncertain
                        )
                    {
                        return Err(ObservationFailure::ContextMismatch);
                    }
                    let effect = *actor
                        .pending()
                        .iter()
                        .find(|effect| effect.spec == EffectSpec::ReleaseResources)
                        .ok_or(ObservationFailure::ContextMismatch)?;
                    (effect, EffectReceipt::ResourceUncertain, true)
                }
                (OperationPhase::Cancelling, VerificationDispatchStage::OutcomeUnknown) => {
                    return Err(ObservationFailure::AwaitingActor);
                }
                _ => return Err(ObservationFailure::ContextMismatch),
            };
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt,
            }))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        let step = actor.run_once();
        if cleanup_uncertain {
            owned.cleanup_stage = CleanupStage::Uncertain;
        }
        entry.verification_dispatch = VerificationDispatchStage::Delivered;
        entry.verification_dispatch_step = Some(step.clone());
        drop(entries);
        Ok(step)
    }

    /// Passes the saved transition along with the effects; failure does not erase it.
    #[allow(dead_code)]
    pub(crate) fn handoff_verification_send_failure(
        &self,
        operation: OperationId,
        effects: &crate::effect_owner::EffectOwner,
    ) -> Result<(), crate::effect_owner::EffectOwnerError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&operation)
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        let step = entry
            .verification_dispatch_step
            .as_ref()
            .ok_or(crate::effect_owner::EffectOwnerError::NotPending)?;
        effects.accept_step(operation, step)?;
        entry.verification_dispatch_step = None;
        Ok(())
    }

    /// Transmits a proven command failure; cleaning remains a separate effect.
    pub(crate) fn apply_verification_rejection(
        &self,
        context: &crate::provider_verification::VerificationContext,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        use carosign_engine::{DomainInput, EffectSpec, OperationPhase};
        use carosign_runtime::OperationMsg;
        let mut guard = self.checkout(context.operation)?;
        let actor = &mut guard.operation_mut().actor;
        let state = actor.state();
        if state.common.phase != OperationPhase::Running
            || state.common.context.operation_id != context.operation
            || state.common.context.authenticator.id != context.authenticator
            || state.common.context.authenticator.device_epoch != context.epoch
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker || worker.device_epoch != context.epoch
            })
            || state
                .common
                .command
                .is_none_or(|command| command.id != context.command)
            || !actor
                .pending()
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendVerify)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if actor.has_work() || actor.has_control_work() {
            return Err(ObservationFailure::AwaitingActor);
        }
        actor
            .post(OperationMsg::Domain(DomainInput::ProviderFailed))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        Ok(actor.run_once())
    }

    /// Passes confirmation already verified by the session owner with the channel.
    pub(crate) fn apply_verification_ack(
        &self,
        context: &crate::provider_verification::VerificationContext,
    ) -> Result<carosign_runtime::operation::actor::Step, ObservationFailure> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
        use carosign_runtime::OperationMsg;
        let mut guard = self.checkout(context.operation)?;
        let actor = &mut guard.operation_mut().actor;
        let state = actor.state();
        if state.common.context.operation_id != context.operation
            || state.common.context.authenticator.id != context.authenticator
            || state.common.context.authenticator.device_epoch != context.epoch
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker || worker.device_epoch != context.epoch
            })
            || state
                .common
                .command
                .is_none_or(|command| command.id != context.command)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        let effect = *actor
            .pending()
            .iter()
            .find(|effect| effect.spec == EffectSpec::SendVerify)
            .ok_or(ObservationFailure::ContextMismatch)?;
        actor
            .post(OperationMsg::Domain(DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::SendAccepted,
            }))
            .map_err(|_| ObservationFailure::MailboxFull)?;
        Ok(actor.run_once())
    }

    pub(crate) fn verification_ack_processed(
        &self,
        context: &crate::provider_verification::VerificationContext,
    ) -> Result<(), ObservationFailure> {
        use carosign_engine::{EffectSpec, OperationPhase};
        let mut guard = self.checkout(context.operation)?;
        let actor = &mut guard.operation_mut().actor;
        let state = actor.state();
        if state.common.context.authenticator.id != context.authenticator
            || state.common.context.authenticator.device_epoch != context.epoch
            || state.common.worker.is_none_or(|worker| {
                worker.instance != context.worker || worker.device_epoch != context.epoch
            })
            || state
                .common
                .command
                .is_none_or(|command| command.id != context.command)
            || state.common.phase != OperationPhase::Running
            || state.common.cancel_requested
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if actor.has_work()
            || actor
                .pending()
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendVerify)
        {
            return Err(ObservationFailure::AwaitingActor);
        }
        Ok(())
    }

    pub(crate) fn checkout(
        &self,
        id: OperationId,
    ) -> Result<OperationGuard<'_>, ObservationFailure> {
        self.checkout_inner(id, false)
    }

    fn checkout_for_control(
        &self,
        id: OperationId,
    ) -> Result<OperationGuard<'_>, ObservationFailure> {
        self.checkout_inner(id, true)
    }

    fn checkout_inner(
        &self,
        id: OperationId,
        allow_deferred: bool,
    ) -> Result<OperationGuard<'_>, ObservationFailure> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get_mut(&id)
            .ok_or(ObservationFailure::UnknownOperation)?;
        if !allow_deferred
            && entry
                .operation
                .as_ref()
                .is_some_and(|operation| operation.deferred_control_step.is_some())
        {
            return Err(ObservationFailure::Busy);
        }
        let operation = entry.operation.take().ok_or(ObservationFailure::Busy)?;
        Ok(OperationGuard {
            registry: self,
            id,
            operation: Some(operation),
        })
    }
}

fn mutation_binding_snapshot(
    owned: &OwnedOperation,
    request: &carosign_runtime::ports::MutationArmRequest,
    now: carosign_core::operation::context::BootInstant,
) -> Result<
    (
        crate::resources::CleanupScope,
        ValidatedGrant,
        carosign_engine::DomainInput,
    ),
    ObservationFailure,
> {
    use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase, WorkerRef};
    let actor = &owned.actor;
    let state = actor.state();
    if state.common.context.deadline.is_expired_at(now)
        || now.boot() != owned.grant.boot()
        || now.instant_ns() >= owned.grant.deadline_ns()
    {
        return Err(ObservationFailure::DeadlineExpired);
    }
    if actor.has_work() || actor.has_control_work() {
        return Err(ObservationFailure::AwaitingActor);
    }
    let lease = owned
        .lease
        .as_ref()
        .ok_or(ObservationFailure::ContextMismatch)?;
    let owner = lease.owner();
    let mutation = state
        .mutation()
        .ok_or(ObservationFailure::ContextMismatch)?;
    if !request.is_consistent()
        || !request.matches_operation_label(state)
        || state.common.phase != OperationPhase::Arming
        || state.common.cancel_requested
        || !owned.lease_matches_request
        || !crate::run::grant_matches_operation(actor, &owned.grant)
        || state.common.context.operation_id != request.operation
        || state.common.context.subject != request.logical_plan.subject()
        || state.common.context.authenticator.id != request.logical_plan.authenticator()
        || mutation.mutation != request.arm.mutation
        || mutation.plan.as_ref() != Some(&request.logical_plan)
        || lease.lease_id() != request.lease
        || lease.resources().is_empty()
        || owner.operation != request.operation
        || owner.authenticator != state.common.context.authenticator
        || owner.worker != request.arm.worker
        || owner.authenticator.device_epoch != request.arm.epoch
    {
        return Err(ObservationFailure::ContextMismatch);
    }
    let mut pending = actor
        .pending()
        .iter()
        .filter(|effect| effect.spec == EffectSpec::PersistDispatchArmed);
    let arm = *pending.next().ok_or(ObservationFailure::ContextMismatch)?;
    if pending.next().is_some() {
        return Err(ObservationFailure::ContextMismatch);
    }
    let input = DomainInput::BindMutationDispatch {
        effect: arm,
        lease: request.lease,
        worker: WorkerRef {
            instance: request.arm.worker,
            device_epoch: request.arm.epoch,
        },
        command: request.arm.command,
    };
    // Both forecasts are clean and are not evidence of a stable record.
    let bound = carosign_engine::decide(state, input.clone())
        .map_err(|_| ObservationFailure::ContextMismatch)?;
    let armed = carosign_engine::decide(
        &bound.next,
        DomainInput::Receipt {
            effect: arm,
            receipt: EffectReceipt::DispatchArmedDurable,
        },
    )
    .map_err(|_| ObservationFailure::ContextMismatch)?;
    if armed.effects.as_slice() != [request.effect] {
        return Err(ObservationFailure::ContextMismatch);
    }
    Ok((
        crate::resources::CleanupScope::of(lease),
        owned.grant.clone(),
        input,
    ))
}

#[path = "operation_registry.tests.rs"]
#[cfg(test)]
mod tests;
