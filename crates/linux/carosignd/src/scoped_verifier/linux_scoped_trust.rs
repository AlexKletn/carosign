use super::{
    EvidenceRefusal, ScopedAssertionContext, ScopedChallenges, ScopedProviderAssertion,
    ScopedTrust, TimeSource,
};

/// Verifies the source and scope of the Linux content.
/// It is created only after admitting the operation and registering its call.
pub(crate) struct LinuxScopedTrust<'a> {
    registry: &'a crate::operations::OperationRegistry,
    policy: &'a crate::linux_policy::LinuxPolicy,
    cleanup: &'a crate::resources::SupervisorCleanup,
    prepared: &'a crate::service::PreparedClientVerification,
    target: crate::verification_owner::VerificationFlowTarget,
    pub(super) operation: carosign_core::ids::OperationId,
    pub(super) context: ScopedAssertionContext,
}

impl<'a> LinuxScopedTrust<'a> {
    /// Registers a one-time call only for an already admitted operation.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register(
        registry: &'a crate::operations::OperationRegistry,
        policy: &'a crate::linux_policy::LinuxPolicy,
        cleanup: &'a crate::resources::SupervisorCleanup,
        prepared: &'a crate::service::PreparedClientVerification,
        target: crate::verification_owner::VerificationFlowTarget,
        operation: carosign_core::ids::OperationId,
        challenges: &ScopedChallenges,
        clock: &dyn TimeSource,
    ) -> Result<Self, EvidenceRefusal> {
        let context = {
            let mut guard = registry.checkout(operation).map_err(|_| {
                eprintln!("carosignd: доверенный контекст: операция недоступна");
                EvidenceRefusal::Unverified
            })?;
            if guard.delivery_invalidated() {
                eprintln!("carosignd: доверенный контекст: доставка отозвана");
                return Err(EvidenceRefusal::Unverified);
            }
            let owned = guard.operation_mut();
            let op = &owned.actor.state().common.context;
            if !owned.belongs_to(&prepared.caller)
                || owned.grant.id() != prepared.grant.id()
                || owned.grant.issuer() != prepared.grant.issuer()
                || owned.grant.scope() != prepared.grant.scope()
                || op.operation_id != operation
                || op.authenticator != target.authenticator
                || op.purpose != target.purpose
            {
                eprintln!("carosignd: доверенный контекст: параметры операции не совпадают");
                return Err(EvidenceRefusal::Unverified);
            }
            ScopedAssertionContext {
                subject: op.subject,
                authenticator: op.authenticator,
                purpose: op.purpose,
                recipient: target.recipient,
                challenge: target.challenge,
                deadline: op.deadline,
            }
        };
        let trust = Self {
            registry,
            policy,
            cleanup,
            prepared,
            target,
            operation,
            context,
        };
        trust.confirm_live().map_err(|error| {
            eprintln!("carosignd: доверенный контекст: проверка действительности: {error:?}");
            error
        })?;
        challenges
            .register(operation, trust.context.clone(), clock.now())
            .map_err(|error| {
                eprintln!("carosignd: доверенный контекст: регистрация вызова: {error:?}");
                EvidenceRefusal::Unverified
            })?;
        Ok(trust)
    }

    fn confirm_live(&self) -> Result<(), EvidenceRefusal> {
        use carosign_core::authorization::GrantScope;
        if !matches!(self.prepared.grant.scope(), GrantScope::Verify { subject, authenticator, purpose }
            if *subject == self.context.subject && *authenticator == self.context.authenticator.id && *purpose == self.context.purpose)
            || self.prepared.cycle.caller != self.prepared.caller
            || self.prepared.cycle.scope != *self.prepared.grant.scope()
            || !self.cleanup.confirms_worker(self.target.worker)
        {
            eprintln!(
                "carosignd: доверенный контекст: область допуска или работник не подтверждены"
            );
            return Err(EvidenceRefusal::Unverified);
        }
        self.policy
            .revalidate_now(&self.prepared.caller, &self.prepared.grant)
            .map_err(|error| {
                eprintln!(
                    "carosignd: доверенный контекст: повторная проверка политики: {error:?}"
                );
                EvidenceRefusal::Unverified
            })?;
        let mut guard = self.registry.checkout(self.operation).map_err(|_| {
            eprintln!(
                "carosignd: доверенный контекст: операция недоступна при повторной проверке"
            );
            EvidenceRefusal::Unverified
        })?;
        if guard.delivery_invalidated() {
            eprintln!("carosignd: доверенный контекст: доставка отозвана при повторной проверке");
            return Err(EvidenceRefusal::Unverified);
        }
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        let op = &state.common.context;
        if !owned.belongs_to(&self.prepared.caller)
            || owned.grant.id() != self.prepared.grant.id()
            || owned.grant.issuer() != self.prepared.grant.issuer()
            || owned.grant.scope() != self.prepared.grant.scope()
            || op.operation_id != self.operation
            || op.subject != self.context.subject
            || op.authenticator != self.context.authenticator
            || op.purpose != self.context.purpose
            || op.deadline != self.context.deadline
            || state.common.cancel_requested
            || owned.actor.has_control_work()
        {
            eprintln!(
                "carosignd: доверенный контекст: состояние операции изменилось или поступила отмена"
            );
            return Err(EvidenceRefusal::Unverified);
        }
        Ok(())
    }
}

impl ScopedTrust for LinuxScopedTrust<'_> {
    fn confirm(
        &self,
        assertion: &ScopedProviderAssertion,
        context: &ScopedAssertionContext,
    ) -> Result<(), EvidenceRefusal> {
        if context != &self.context
            || assertion.operation() != self.operation
            || assertion.worker() != self.target.worker
            || assertion.command() != self.target.command
            || assertion.epoch() != self.context.authenticator.device_epoch
            || assertion.permitted().scoped_digest()
                != self.prepared.cycle.permitted.scoped_digest()
        {
            return Err(EvidenceRefusal::Unverified);
        }
        self.confirm_live()
    }
}
