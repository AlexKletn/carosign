use super::{
    AdvanceOutcome, DriveOutcome, Duration, EffectOwner, FlowError, FlowProgress, FlowStage,
    Future, MatchedEvidenceOwner, ObservationProgress, OperationId, OperationRegistry,
    PositiveCompletion, PositiveCompletionError, PumpEvent, StartError, Step,
    VerificationFlowTarget, VerificationOwner, VerificationStart, VerificationStartPorts,
};

/// The internal owner of a negative scenario of one accepted operation.
/// Policy, channel, mediator and hours are provided by the composition of the service.
pub(crate) struct VerificationFlow {
    registry: std::sync::Arc<OperationRegistry>,
    clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
    pub(super) context: carosign_adapters::provider::VerificationFrameContext,
    caller: carosign_runtime::ports::CallerContext,
    effects: EffectOwner,
    scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
    _control: std::sync::Arc<crate::control_watch::ControlOwner>,
    _watch: crate::control_watch::WatchHandle,
    _revocation: crate::control_watch::RevocationSender,
    // The exchange remains with the operation until the cleanup is confirmed; a coincidence
    // does not mean the end of the service provider.
    _matched_provider: Option<VerificationOwner>,
    scoped_assessment: Option<carosign_runtime::ports::ScopedAssertionAssessment>,
    stage: FlowStage,
}

impl VerificationFlow {
    /// Allows resources to prepare the channel and returns them to the exact area.
    /// The issue remains with the actor; only its description is transmitted outwardly.
    pub(crate) async fn prepare_channel<S, A, I, T>(
        &mut self,
        service: &crate::service::Service<'_, S, A, I, T>,
        prepared: &crate::service::PreparedClientVerification,
    ) -> Result<crate::resources::CleanupScope, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        A: carosign_runtime::ports::PolicyAuthority,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
    {
        if !matches!(self.stage, FlowStage::Admitted)
            || !std::ptr::eq(service.operations, self.registry.as_ref())
            || !std::ptr::addr_eq(
                service.clock as &dyn carosign_runtime::ports::TimeSource,
                self.clock.as_ref(),
            )
            || prepared.caller != self.caller
        {
            return Err(FlowError::ContextMismatch);
        }
        self.stage = FlowStage::Acquiring;
        let outcome = service
            .prepare_admitted_dispatch(prepared, self.context.operation, self.context.worker)
            .await
            .map_err(FlowError::Operation)?;
        if !matches!(
            outcome,
            crate::run::RunOutcome::Waiting {
                phase: carosign_engine::OperationPhase::Running
            }
        ) {
            self.stage = FlowStage::Failed;
            return Err(FlowError::Preparation(outcome));
        }
        let scope = self.retained_scope()?.ok_or(FlowError::ContextMismatch)?;
        self.stage = FlowStage::Starting(
            VerificationStart::bind(&self.registry, self.context.clone())
                .map_err(FlowError::Start)?,
        );
        Ok(scope)
    }

    pub(crate) fn retained_scope(
        &self,
    ) -> Result<Option<crate::resources::CleanupScope>, FlowError> {
        let mut guard = self
            .registry
            .checkout(self.context.operation)
            .map_err(FlowError::Operation)?;
        let Some(lease) = guard.operation_mut().lease.as_ref() else {
            return Ok(None);
        };
        if lease.owner().operation != self.context.operation
            || lease.owner().worker != self.context.worker
            || lease.owner().authenticator != self.context.authenticator
        {
            return Err(FlowError::ContextMismatch);
        }
        Ok(Some(crate::resources::CleanupScope::of(lease)))
    }

    /// Receives limited approval to an independent port.
    /// The success of a port does not yet allow entry or positive delivery: their trusted
    /// owner and completion without byte material are not connected.
    #[allow(dead_code)]
    pub(crate) async fn evaluate_scoped_assertion(
        &mut self,
        verifier: &dyn carosign_runtime::ports::EvidencePort,
        requirements: &carosign_evidence::requirements::EvidenceRequirements,
    ) -> Result<carosign_runtime::ports::ScopedAssertionAssessment, FlowError> {
        if !matches!(self.stage, FlowStage::MatchedHeld) {
            return Err(FlowError::InvalidStage);
        }
        let context = {
            let mut guard = self
                .registry
                .checkout(self.context.operation)
                .map_err(FlowError::Operation)?;
            let op = &guard.operation_mut().actor.state().common.context;
            if op.authenticator != self.context.authenticator || op.purpose != self.context.purpose
            {
                return Err(FlowError::ContextMismatch);
            }
            carosign_runtime::ports::ScopedAssertionContext {
                subject: op.subject,
                authenticator: op.authenticator,
                purpose: op.purpose,
                recipient: self.context.recipient,
                challenge: self.context.challenge,
                deadline: op.deadline,
            }
        };
        if requirements.purpose != context.purpose
            || !requirements.accepts(carosign_core::assurance::AssuranceClass::SoftwareAsserted)
            || !context.challenge.is_single_use()
        {
            return Err(FlowError::ContextMismatch);
        }
        let assertion = self
            .registry
            .take_observed_scoped_assertion(self.context.operation, self.clock.now())
            .map_err(FlowError::Operation)?;
        if assertion.worker() != self.context.worker
            || assertion.epoch() != self.context.authenticator.device_epoch
            || assertion.command() != self.context.command
            || assertion.operation() != self.context.operation
        {
            self.stage = FlowStage::Failed;
            return Err(FlowError::ContextMismatch);
        }
        let permitted = assertion.permitted().clone();
        let matched = assertion.matched();
        self.stage = FlowStage::ScopedAssertionEvaluating;
        let assessment = verifier
            .evaluate_scoped_assertion(assertion, &context, requirements)
            .await
            .map_err(FlowError::ScopedEvidence)?;
        let checked = assessment.assertion();
        if !self
            .registry
            .owns_scoped_assertion(self.context.operation, &assessment)
            || checked.worker != self.context.worker
            || checked.authenticator != self.context.authenticator
            || checked.command != self.context.command
            || checked.operation != self.context.operation
            || checked.permitted_digest != permitted.scoped_digest()
            || checked.permitted
                != permitted
                    .bindings()
                    .iter()
                    .map(|b| b.enrollment)
                    .collect::<Vec<_>>()
            || checked.matched != matched
            || checked.subject != context.subject
            || checked.purpose != context.purpose
            || checked.recipient != context.recipient
            || checked.challenge != context.challenge
            || checked.deadline != context.deadline
            || assessment.context() != &context
            || !requirements.accepts(assessment.class())
            || !assessment.still_valid(self.clock.now())
            || context.deadline.is_expired_at(self.clock.now())
        {
            self.stage = FlowStage::Failed;
            return Err(FlowError::ContextMismatch);
        }
        self.stage = FlowStage::ScopedAssertionEvaluated;
        Ok(assessment)
    }

    /// It transmits the recording to the actor, holding the caller's grade, and it's not an
    /// independent evaluation receipt, it's not permission to deliver the result.
    pub(crate) fn stage_scoped_assessment(
        &mut self,
        assessment: &carosign_runtime::ports::ScopedAssertionAssessment,
    ) -> Result<(), FlowError> {
        if !matches!(self.stage, FlowStage::ScopedAssertionEvaluated) {
            return Err(FlowError::InvalidStage);
        }
        self.registry
            .stage_scoped_assertion(self.context.operation, assessment, self.clock.now())
            .map_err(FlowError::Operation)?;
        self.stage = FlowStage::ScopedAssertionStaged;
        Ok(())
    }

    /// Transfers the retained step and a single evaluation to the effects owner.
    /// After that, the issue is released and the record remains for DestroyEvidence.
    pub(crate) fn apply_scoped_assessment(
        &mut self,
        assessment: carosign_runtime::ports::ScopedAssertionAssessment,
    ) -> Result<(), FlowError> {
        if !matches!(self.stage, FlowStage::ScopedAssertionStaged) {
            return Err(FlowError::InvalidStage);
        }
        self.registry
            .handoff_matched_evidence(self.context.operation, &self.effects)
            .map_err(FlowError::Handoff)?;
        let effect = match self.effects.step(self.context.operation) {
            Some(Step::Advanced { effects, .. }) => effects
                .into_iter()
                .find(|e| e.spec == carosign_engine::EffectSpec::VerifyEvidence)
                .ok_or(FlowError::InvalidStage)?,
            _ => return Err(FlowError::InvalidStage),
        };
        self.effects
            .acknowledge_step(self.context.operation)
            .map_err(FlowError::Handoff)?;
        let mut claim = self
            .effects
            .claim_effect(self.context.operation, effect)
            .map_err(FlowError::Handoff)?;
        claim.mark_started();
        self.registry
            .apply_scoped_assertion(
                self.context.operation,
                effect,
                &assessment,
                self.clock.now(),
            )
            .map_err(FlowError::Operation)?;
        claim.acknowledge();
        self.registry
            .handoff_evidence_evaluation(self.context.operation, &self.effects)
            .map_err(FlowError::Handoff)?;
        self.scoped_assessment = Some(assessment);
        self.stage = FlowStage::MatchedEvaluated;
        Ok(())
    }

    /// Transmits a one-time rating to the actor and prepares a positive delivery.
    /// Sources of trust and binding of the connection provides a caller composition.
    pub(crate) async fn prepare_scoped_positive<F: Future<Output = ()>>(
        &mut self,
        assessment: carosign_runtime::ports::ScopedAssertionAssessment,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        cleanup_timeout: impl Fn() -> F,
    ) -> Result<(), FlowError> {
        self.stage_scoped_assessment(&assessment)?;
        self.apply_scoped_assessment(assessment)?;
        self.prepare_positive(authority, broker, cleanup_timeout)
            .await
    }
    /// Admission with the image that issued the grant: monitoring the policy and verifying
    /// the owner cannot be replaced by observing another file.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_linux<S, I, T>(
        service: &crate::service::Service<'_, S, crate::linux_policy::LinuxPolicy, I, T>,
        prepared: &crate::service::PreparedClientVerification,
        binding: Option<&crate::operations::TrustedCallerBinding>,
        connection: crate::operations::ConnectionHandle,
        capacity: usize,
        registry: std::sync::Arc<OperationRegistry>,
        clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
        control: std::sync::Arc<crate::control_watch::ControlOwner>,
        scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
        target: VerificationFlowTarget,
        watch_lifetime: Duration,
    ) -> Result<Self, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
    {
        let policy_watch = service
            .authority
            .operation_watch_for(&prepared.caller, &prepared.grant)
            .map_err(FlowError::Authority)?;
        Self::admit_observing(
            service,
            prepared,
            binding,
            connection,
            capacity,
            registry,
            clock,
            control,
            scheduler,
            target,
            watch_lifetime,
            Some(policy_watch),
        )
    }

    /// The former internal assembly with the direct owner of the picture.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_snapshot<S, I, T>(
        service: &crate::service::Service<'_, S, crate::policy_authority::SnapshotAuthority, I, T>,
        prepared: &crate::service::PreparedClientVerification,
        binding: Option<&crate::operations::TrustedCallerBinding>,
        connection: crate::operations::ConnectionHandle,
        capacity: usize,
        registry: std::sync::Arc<OperationRegistry>,
        clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
        control: std::sync::Arc<crate::control_watch::ControlOwner>,
        scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
        target: VerificationFlowTarget,
        watch_lifetime: Duration,
    ) -> Result<Self, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
    {
        let policy_watch = service
            .authority
            .operation_watch_for(&prepared.caller, &prepared.grant)
            .map_err(FlowError::Authority)?;
        Self::admit_observing(
            service,
            prepared,
            binding,
            connection,
            capacity,
            registry,
            clock,
            control,
            scheduler,
            target,
            watch_lifetime,
            Some(policy_watch),
        )
    }

    pub(crate) fn operation(&self) -> OperationId {
        self.context.operation
    }

    /// Prepares confirmation of acceptance with a real term of the saved operation.
    /// You need to transfer it to the client before the launch of the provider and the
    /// terminal event.
    pub(crate) fn accepted_reply(&self) -> Result<crate::answer::Reply, FlowError> {
        if !matches!(self.stage, FlowStage::Admitted) {
            return Err(FlowError::InvalidStage);
        }
        let mut guard = self
            .registry
            .checkout(self.context.operation)
            .map_err(FlowError::Operation)?;
        if guard.delivery_invalidated() {
            return Err(FlowError::Operation(
                crate::operations::ObservationFailure::AwaitingActor,
            ));
        }
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        if !owned.belongs_to(&self.caller)
            || state.common.context.operation_id != self.context.operation
            || state.common.context.authenticator != self.context.authenticator
            || state.common.phase != carosign_engine::OperationPhase::New
            || owned.actor.has_control_work()
        {
            return Err(FlowError::ContextMismatch);
        }
        let deadline = state.common.context.deadline;
        Ok(crate::answer::Reply::Accepted {
            operation: self.context.operation,
            boot: deadline.boot(),
            instant_ns: deadline.instant_ns(),
        })
    }

    /// Atomically, one operation is allowed and independent oversight is initiated before
    /// resources are requested, and the binding source and session selection are verified by
    /// the composition.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit<S, A, I, T>(
        service: &crate::service::Service<'_, S, A, I, T>,
        prepared: &crate::service::PreparedClientVerification,
        binding: Option<&crate::operations::TrustedCallerBinding>,
        connection: crate::operations::ConnectionHandle,
        capacity: usize,
        registry: std::sync::Arc<OperationRegistry>,
        clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
        control: std::sync::Arc<crate::control_watch::ControlOwner>,
        scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
        target: VerificationFlowTarget,
        watch_lifetime: Duration,
    ) -> Result<Self, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        A: carosign_runtime::ports::PolicyAuthority,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
    {
        Self::admit_observing(
            service,
            prepared,
            binding,
            connection,
            capacity,
            registry,
            clock,
            control,
            scheduler,
            target,
            watch_lifetime,
            None,
        )
    }

    /// Surveillance alone does not confirm the origin of the policy: it is transmitted by the
    /// composition that checked the same picture before issuing the rights.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_observing<S, A, I, T>(
        service: &crate::service::Service<'_, S, A, I, T>,
        prepared: &crate::service::PreparedClientVerification,
        binding: Option<&crate::operations::TrustedCallerBinding>,
        connection: crate::operations::ConnectionHandle,
        capacity: usize,
        registry: std::sync::Arc<OperationRegistry>,
        clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
        control: std::sync::Arc<crate::control_watch::ControlOwner>,
        scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
        target: VerificationFlowTarget,
        watch_lifetime: Duration,
        policy_watch: Option<carosign_linux::policy::PolicyChangeWatch>,
    ) -> Result<Self, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        A: carosign_runtime::ports::PolicyAuthority,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
    {
        if !std::ptr::eq(service.operations, registry.as_ref())
            || !std::ptr::addr_eq(
                service.clock as &dyn carosign_runtime::ports::TimeSource,
                clock.as_ref(),
            )
            || !matches!(prepared.grant.scope(), carosign_core::authorization::GrantScope::Verify {
                subject, authenticator, purpose
            } if *subject == target.provider_subject && *authenticator == target.authenticator.id && *purpose == target.purpose)
        {
            return Err(FlowError::ContextMismatch);
        }
        let operation = match service.admit_client_verification(
            prepared,
            target.authenticator.device_epoch,
            binding,
            connection,
            capacity,
        ) {
            crate::operations::IdempotentAdmission::Created(operation) => operation,
            other => return Err(FlowError::Admission(other)),
        };
        let context = carosign_adapters::provider::VerificationFrameContext {
            worker: target.worker,
            command: target.command,
            operation,
            authenticator: target.authenticator,
            provider_subject: target.provider_subject,
            uid: target.uid,
            purpose: target.purpose,
            challenge: target.challenge,
            recipient: target.recipient,
        };
        match Self::new_observing(
            registry.clone(),
            clock,
            control,
            scheduler,
            context,
            prepared.caller.clone(),
            watch_lifetime,
            policy_watch,
        ) {
            Ok(flow) => Ok(flow),
            Err(error) => {
                let _ = registry.revoke_verification(operation);
                Err(error)
            }
        }
    }

    /// The source of the recall signal is not a confirmation of the cleaning.
    pub(crate) fn revoke(&self) -> Result<(), crate::control_watch::SignalError> {
        self._revocation.revoke()
    }

    pub(crate) fn cancellation_sender(&self) -> crate::control_watch::RevocationSender {
        self._revocation.clone()
    }

    pub(crate) async fn cleanup_after_failure(
        &mut self,
        broker: &crate::resources::SupervisorCleanup,
    ) -> Result<(), FlowError> {
        self._watch.finish();
        self.drain_control(broker, std::future::pending).await?;
        Ok(())
    }

    /// Delayed control: the fence is held until confirmed cleaning, even if the receiver of
    /// the signal was completed before the operation.
    pub(crate) async fn drain_control<F: Future<Output = ()>>(
        &mut self,
        broker: &crate::resources::SupervisorCleanup,
        timeout: impl Fn() -> F,
    ) -> Result<bool, FlowError> {
        let Some(claim) = self._control.claim(self.context.operation) else {
            return Ok(false);
        };
        match claim.result() {
            crate::control_watch::WatchResult::Step(step) => self
                .effects
                .accept_step(self.context.operation, step)
                .map_err(FlowError::Handoff)?,
            crate::control_watch::WatchResult::Deferred => self
                .registry
                .transfer_deferred_control(self.context.operation, &self.effects)
                .map_err(|_| FlowError::ContextMismatch)?,
            crate::control_watch::WatchResult::Failed(_) => return Err(FlowError::ContextMismatch),
        }
        claim.acknowledge();
        let id = self.context.operation;
        let Some(Step::Advanced { effects, .. }) = self.effects.step(id) else {
            return Err(FlowError::ContextMismatch);
        };
        if let Some(cancel) = effects
            .iter()
            .find(|effect| effect.spec == carosign_engine::EffectSpec::RequestCancel)
            .copied()
        {
            // Only stop known processes are confirmed; the state of the device separately
            // checks for the next cleaning effect.
            broker.interrupt().map_err(|_| FlowError::ContextMismatch)?;
            self.effects
                .acknowledge_step(id)
                .map_err(FlowError::Handoff)?;
            let claim = self
                .effects
                .claim_effect(id, cancel)
                .map_err(FlowError::Handoff)?;
            let step = {
                let mut guard = self.registry.checkout(id).map_err(FlowError::Operation)?;
                if !guard.delivery_invalidated() {
                    return Err(FlowError::ContextMismatch);
                }
                let actor = &mut guard.operation_mut().actor;
                for effect in effects
                    .iter()
                    .filter(|effect| effect.spec == carosign_engine::EffectSpec::SuppressResult)
                {
                    actor
                        .post(carosign_runtime::OperationMsg::Domain(
                            carosign_engine::DomainInput::Receipt {
                                effect: *effect,
                                receipt: carosign_engine::EffectReceipt::ResultSuppressed,
                            },
                        ))
                        .map_err(|_| FlowError::ContextMismatch)?;
                    loop {
                        match actor.run_once() {
                            Step::Advanced { effects, .. } if effects.is_empty() => {
                                if !actor.has_work() {
                                    break;
                                }
                            }
                            _ => return Err(FlowError::ContextMismatch),
                        }
                    }
                }
                actor
                    .post(carosign_runtime::OperationMsg::Domain(
                        carosign_engine::DomainInput::Receipt {
                            effect: cancel,
                            receipt: carosign_engine::EffectReceipt::CancelWorkerStopped,
                        },
                    ))
                    .map_err(|_| FlowError::ContextMismatch)?;
                loop {
                    let step = actor.run_once();
                    match &step {
                        Step::Advanced { effects, .. }
                            if effects.is_empty() && actor.has_work() =>
                        {
                            continue;
                        }
                        _ => break step,
                    }
                }
            };
            claim.acknowledge();
            for effect in effects
                .iter()
                .filter(|effect| effect.spec == carosign_engine::EffectSpec::SuppressResult)
            {
                self.effects
                    .claim_effect(id, *effect)
                    .map_err(FlowError::Handoff)?
                    .acknowledge();
            }
            self.effects
                .accept_step(id, &step)
                .map_err(FlowError::Handoff)?;
        }
        self.stage = FlowStage::SendFailureDrive;
        let result = self
            .effects
            .drive_negative_scheduled(
                &self.registry,
                self.context.operation,
                &self.scheduler,
                broker,
                timeout,
            )
            .await
            .map_err(FlowError::Drive)?;
        if matches!(result, DriveOutcome::TerminalPending) {
            self.stage = FlowStage::Delivery;
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn control_observed(&self) -> bool {
        self._control.claim(self.context.operation).is_some()
    }

    pub(crate) fn new(
        registry: std::sync::Arc<OperationRegistry>,
        clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
        control: std::sync::Arc<crate::control_watch::ControlOwner>,
        scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
        context: carosign_adapters::provider::VerificationFrameContext,
        caller: carosign_runtime::ports::CallerContext,
        watch_lifetime: Duration,
    ) -> Result<Self, FlowError> {
        Self::new_observing(
            registry,
            clock,
            control,
            scheduler,
            context,
            caller,
            watch_lifetime,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_observing(
        registry: std::sync::Arc<OperationRegistry>,
        clock: std::sync::Arc<dyn carosign_runtime::ports::TimeSource>,
        control: std::sync::Arc<crate::control_watch::ControlOwner>,
        scheduler: std::sync::Arc<std::sync::Mutex<carosign_runtime::scheduler::Scheduler>>,
        context: carosign_adapters::provider::VerificationFrameContext,
        caller: carosign_runtime::ports::CallerContext,
        watch_lifetime: Duration,
        policy_watch: Option<carosign_linux::policy::PolicyChangeWatch>,
    ) -> Result<Self, FlowError> {
        registry
            .check_verification_flow_owner(&context, &caller, clock.now())
            .map_err(FlowError::Operation)?;
        let (watch, revocation) = match control.start_observing(
            registry.clone(),
            context.operation,
            clock.clone(),
            watch_lifetime,
            policy_watch,
        ) {
            Ok(watch) => watch,
            Err(error) => {
                if error != crate::control_watch::StartError::Duplicate {
                    let _ = registry.revoke_verification(context.operation);
                }
                return Err(FlowError::Watch(error));
            }
        };
        Ok(Self {
            registry,
            clock,
            context,
            caller,
            effects: EffectOwner::default(),
            scheduler,
            _control: control,
            _watch: watch,
            _revocation: revocation,
            _matched_provider: None,
            scoped_assessment: None,
            stage: FlowStage::Admitted,
        })
    }

    /// If you refuse or interrupt, the work started is not replayed; the steps saved by the
    /// registry and the leasing remain available to the owner of the restoration.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run<S, A, I, T, F>(
        &mut self,
        service: &crate::service::Service<'_, S, A, I, T>,
        prepared: &crate::service::PreparedClientVerification,
        transport: Option<carosign_linux_ipc::provider::transport::ProviderTransport>,
        connection: &crate::server::Connection,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        timeout: Duration,
        cleanup_timeout: impl Fn() -> F,
    ) -> Result<FlowProgress, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        A: carosign_runtime::ports::PolicyAuthority,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
        F: Future<Output = ()>,
    {
        self.run_channel(
            service,
            prepared,
            transport.map(crate::provider_verification::VerificationTransport::Direct),
            connection,
            broker,
            timeout,
            cleanup_timeout,
        )
        .await
    }

    /// Executes the same machine through a held straight channel or tunnel.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_channel<S, A, I, T, F>(
        &mut self,
        service: &crate::service::Service<'_, S, A, I, T>,
        prepared: &crate::service::PreparedClientVerification,
        transport: Option<crate::provider_verification::VerificationTransport>,
        connection: &crate::server::Connection,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        timeout: Duration,
        cleanup_timeout: impl Fn() -> F,
    ) -> Result<FlowProgress, FlowError>
    where
        S: carosign_persistence::store::MetadataStore,
        A: carosign_runtime::ports::PolicyAuthority,
        I: carosign_core::ids::IdSource,
        T: carosign_runtime::ports::TimeSource,
        F: Future<Output = ()>,
    {
        if !std::ptr::eq(service.operations, self.registry.as_ref())
            || !std::ptr::addr_eq(
                service.clock as &dyn carosign_runtime::ports::TimeSource,
                self.clock.as_ref(),
            )
            || prepared.caller != self.caller
        {
            return Err(FlowError::ContextMismatch);
        }
        if matches!(self.stage, FlowStage::Admitted) {
            self.stage = FlowStage::Acquiring;
            let outcome = service
                .prepare_admitted_dispatch(prepared, self.context.operation, self.context.worker)
                .await
                .map_err(FlowError::Operation)?;
            if !matches!(
                outcome,
                crate::run::RunOutcome::Waiting {
                    phase: carosign_engine::OperationPhase::Running
                }
            ) {
                self.stage = FlowStage::Failed;
                return Err(FlowError::Preparation(outcome));
            }
            self.stage = FlowStage::Starting(
                VerificationStart::bind(&self.registry, self.context.clone())
                    .map_err(FlowError::Start)?,
            );
        }
        if let FlowStage::Starting(start) = &mut self.stage {
            let ports = VerificationStartPorts {
                registry: &self.registry,
                effects: &self.effects,
                authority: service.authority,
                clock: service.clock,
                broker,
                scheduler: &self.scheduler,
            };
            let result = start
                .start_channel(
                    &ports,
                    transport.ok_or(FlowError::MissingTransport)?,
                    timeout,
                    &cleanup_timeout,
                )
                .await;
            match result {
                Ok(owner) => self.stage = FlowStage::Receiving(owner),
                Err(StartError::Send(_)) => self.stage = FlowStage::SendFailure,
                Err(error) => return Err(FlowError::Start(error)),
            }
        }
        if matches!(self.stage, FlowStage::SendFailure) {
            let context = crate::provider_verification::VerificationContext {
                operation: self.context.operation,
                worker: self.context.worker,
                command: self.context.command,
                authenticator: self.context.authenticator.id,
                epoch: self.context.authenticator.device_epoch,
            };
            self.registry
                .deliver_verification_send_failure(&context)
                .map_err(FlowError::Operation)?;
            self.stage = FlowStage::SendFailureHandoff;
        }
        if matches!(self.stage, FlowStage::SendFailureHandoff) {
            self.registry
                .handoff_verification_send_failure(self.context.operation, &self.effects)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::SendFailureDrive;
        }
        if matches!(self.stage, FlowStage::SendFailureDrive) {
            let outcome = self
                .effects
                .drive_negative_scheduled(
                    &self.registry,
                    self.context.operation,
                    &self.scheduler,
                    broker,
                    &cleanup_timeout,
                )
                .await
                .map_err(FlowError::Drive)?;
            return Ok(FlowProgress::DispatchFailure(outcome));
        }
        if let FlowStage::Receiving(owner) = &mut self.stage {
            let outcome = owner
                .drive_until_blocked(
                    &self.registry,
                    &self.effects,
                    &self.scheduler,
                    broker,
                    timeout,
                    &cleanup_timeout,
                )
                .await
                .map_err(FlowError::Advance)?;
            if matches!(
                outcome,
                AdvanceOutcome::Other(PumpEvent::Outcome(ObservationProgress::Held(
                    carosign_runtime::operation::actor::ObservationError::EvidenceRequired
                )))
            ) {
                let FlowStage::Receiving(owner) =
                    std::mem::replace(&mut self.stage, FlowStage::MatchedHeld)
                else {
                    unreachable!("владелец удержан во время чтения")
                };
                self._matched_provider = Some(owner);
                return Ok(FlowProgress::Other(outcome));
            }
            if !matches!(
                outcome,
                AdvanceOutcome::Driven(DriveOutcome::TerminalPending)
            ) {
                return Ok(FlowProgress::Other(outcome));
            }
            self.stage = FlowStage::Delivery;
        }
        if matches!(
            self.stage,
            FlowStage::Delivery | FlowStage::DeliveryRetained
        ) {
            let mut delivery = if matches!(self.stage, FlowStage::Delivery) {
                let delivery = service
                    .prepare_negative_delivery(&self.caller, self.context.operation)
                    .await
                    .map_err(FlowError::Delivery)?;
                self.stage = FlowStage::DeliveryRetained;
                delivery
            } else {
                self.registry
                    .reclaim_owner_delivery(self.context.operation, &self.caller)
                    .map_err(FlowError::Delivery)?
            };
            return match delivery
                .try_send(&self.registry, connection, service.authority, service.clock)
                .await
                .map_err(FlowError::Send)?
            {
                crate::operations::OwnerSendProgress::Sent => {
                    self.stage = FlowStage::Finished;
                    Ok(FlowProgress::Sent)
                }
                crate::operations::OwnerSendProgress::WouldBlock => Ok(FlowProgress::WouldBlock),
                crate::operations::OwnerSendProgress::Advanced => {
                    unreachable!("внутренний шаг доставки")
                }
            };
        }
        Err(FlowError::InvalidStage)
    }

    /// Transmits the retained result to an independent verifier. This method does not allow
    /// delivery: further rechecking, cleaning and delivery are owned by the individual owner
    /// of the positive result.
    #[allow(dead_code)]
    pub(crate) async fn evaluate_matched<
        I: carosign_core::ids::IdSource,
        F: Future<Output = ()>,
    >(
        &mut self,
        ids: &I,
        verifier: &dyn carosign_runtime::ports::EvidencePort,
        requirements: &carosign_evidence::requirements::EvidenceRequirements,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        cleanup_timeout: impl Fn() -> F,
    ) -> Result<(), FlowError> {
        if matches!(self.stage, FlowStage::MatchedHeld) {
            let id = ids.new_evidence_id().map_err(FlowError::EvidenceId)?;
            let material = self
                .registry
                .hold_observed_verification_token(self.context.operation, id, self.clock.now())
                .map_err(FlowError::Operation)?;
            self.stage = FlowStage::MatchedStored(material);
        }
        if let FlowStage::MatchedStored(material) = &self.stage {
            self.registry
                .handoff_matched_evidence(self.context.operation, &self.effects)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::MatchedPending(material.clone());
        }
        if let FlowStage::MatchedPending(material) = &self.stage {
            let outcome = self
                .effects
                .drive_negative_scheduled(
                    &self.registry,
                    self.context.operation,
                    &self.scheduler,
                    broker,
                    &cleanup_timeout,
                )
                .await
                .map_err(FlowError::Drive)?;
            if !matches!(outcome, DriveOutcome::ExternalEffect) {
                return Err(FlowError::InvalidStage);
            }
            let effect = match self.effects.step(self.context.operation) {
                Some(Step::Advanced { effects, .. }) => effects
                    .into_iter()
                    .find(|effect| effect.spec == carosign_engine::EffectSpec::VerifyEvidence)
                    .ok_or(FlowError::InvalidStage)?,
                _ => return Err(FlowError::InvalidStage),
            };
            self.stage = FlowStage::MatchedEvaluating(
                MatchedEvidenceOwner::new(self.context.operation, material.clone(), effect)
                    .map_err(FlowError::Evidence)?,
            );
        }
        if let FlowStage::MatchedEvaluating(owner) = &mut self.stage {
            owner
                .drive(
                    &self.registry,
                    &self.effects,
                    verifier,
                    requirements,
                    self.clock.as_ref(),
                )
                .await
                .map_err(FlowError::Evidence)?;
            self.stage = FlowStage::MatchedEvaluated;
            return Ok(());
        }
        Err(FlowError::InvalidStage)
    }

    /// It double-checks the owner before and after the confirmed cleaning, and the loss of a
    /// call that starts prevents it from being re-initiated with another owner.
    /// The resulting step does not yet allow the sending of a positive result.
    #[allow(dead_code)]
    pub(crate) async fn prepare_positive<F: Future<Output = ()>>(
        &mut self,
        authority: &dyn carosign_runtime::ports::PolicyAuthority,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        cleanup_timeout: impl Fn() -> F,
    ) -> Result<(), FlowError> {
        use carosign_engine::EffectSpec;
        let id = self.context.operation;
        if matches!(self.stage, FlowStage::MatchedEvaluated) {
            let effect = self.positive_step(EffectSpec::RevalidateOwner)?;
            self.effects
                .acknowledge_step(id)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::PositiveRevalidating;
            self.effects
                .revalidate_evaluated_owner(
                    &self.registry,
                    id,
                    effect,
                    authority,
                    self.clock.as_ref(),
                )
                .await
                .map_err(FlowError::Operation)?;
            self.stage = FlowStage::PositiveHandoffBeforeCleanup;
        }
        if matches!(self.stage, FlowStage::PositiveHandoffBeforeCleanup) {
            self.registry
                .handoff_owner_revalidation(id, &self.effects)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::PositiveCleanup;
        }
        if matches!(self.stage, FlowStage::PositiveCleanup) {
            let effect = self.positive_step(EffectSpec::ReleaseResources)?;
            self.effects
                .acknowledge_step(id)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::PositiveCleanupStarted;
            self.effects
                .execute_scheduled_cleanup_at(
                    &self.registry,
                    id,
                    effect,
                    &self.scheduler,
                    broker,
                    cleanup_timeout(),
                    self.clock.as_ref(),
                )
                .await
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::PositiveCleanupResult;
        }
        if matches!(self.stage, FlowStage::PositiveCleanupResult) {
            let claim = self
                .effects
                .claim_cleanup(id)
                .ok_or(FlowError::InvalidStage)?;
            self.effects
                .accept_scheduled_cleanup(claim)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::PositiveRevalidateAfterCleanup;
        }
        if matches!(self.stage, FlowStage::PositiveRevalidateAfterCleanup) {
            let effect = self.positive_step(EffectSpec::RevalidateOwner)?;
            self.effects
                .acknowledge_step(id)
                .map_err(FlowError::Handoff)?;
            self.stage = FlowStage::PositiveRevalidatingAfterCleanup;
            self.effects
                .revalidate_cleaned_owner(
                    &self.registry,
                    id,
                    effect,
                    authority,
                    self.clock.as_ref(),
                )
                .await
                .map_err(FlowError::Operation)?;
            self.stage = FlowStage::PositiveHandoffAfterCleanup;
        }
        if matches!(self.stage, FlowStage::PositiveHandoffAfterCleanup) {
            self.registry
                .handoff_owner_revalidation(id, &self.effects)
                .map_err(FlowError::Handoff)?;
            self.positive_step(EffectSpec::DeliverOutcome)?;
            self.stage = FlowStage::PositiveReady;
            return Ok(());
        }
        Err(FlowError::InvalidStage)
    }

    fn positive_step(
        &self,
        expected: carosign_engine::EffectSpec,
    ) -> Result<carosign_engine::PlannedEffect, FlowError> {
        match self.effects.step(self.context.operation) {
            Some(Step::Advanced { effects, .. }) => effects
                .into_iter()
                .find(|effect| effect.spec == expected)
                .ok_or(FlowError::InvalidStage),
            _ => Err(FlowError::InvalidStage),
        }
    }

    /// Transmits the positive effect to the delivery owner, and he has to live until it's
    /// finished or it's abandoned; trust in the connection sets the composition.
    #[allow(dead_code)]
    pub(crate) fn start_positive_delivery(
        &mut self,
    ) -> Result<PositiveCompletion<'_>, PositiveCompletionError> {
        if !matches!(self.stage, FlowStage::PositiveReady) {
            return Err(PositiveCompletionError::InvalidStage);
        }
        let effect = self
            .positive_step(carosign_engine::EffectSpec::DeliverOutcome)
            .map_err(|_| PositiveCompletionError::InvalidStage)?;
        self.effects
            .acknowledge_step(self.context.operation)
            .map_err(PositiveCompletionError::Effect)?;
        self.stage = FlowStage::PositiveDeliveryTaken;
        let mut completion = PositiveCompletion::new(
            &self.registry,
            &self.effects,
            self.context.operation,
            self.caller.clone(),
            effect,
            self.clock.now(),
        )?;
        completion.scoped_assessment = self.scoped_assessment.take();
        Ok(completion)
    }
}
