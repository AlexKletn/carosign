use super::{
    AdvanceError, AdvanceOutcome, DriveOutcome, Duration, EffectOwner, Future, ObservationProgress,
    OperationId, OperationRegistry, OwnerError, OwnerEvent, PumpError, PumpEvent, Step,
    VerificationPump,
};

/// The owner stores the step before taking along with all its effects.
pub(crate) struct VerificationOwner {
    pub(super) operation: OperationId,
    pump: VerificationPump,
    pending: Option<Step>,
    rejection_pending: bool,
    needs_drive: bool,
}

impl VerificationOwner {
    #[cfg(test)]
    pub(crate) fn pending_step(&self) -> Option<&Step> {
        self.pending.as_ref()
    }

    pub(crate) fn new(operation: OperationId, pump: VerificationPump) -> Self {
        Self {
            operation,
            pump,
            pending: None,
            rejection_pending: false,
            needs_drive: false,
        }
    }

    /// Do not read a new frame until the previous step is accepted by the performer.
    pub(crate) fn next(
        &mut self,
        registry: &OperationRegistry,
        timeout: Duration,
    ) -> Result<OwnerEvent, OwnerError> {
        if self.pending.is_some() || self.needs_drive {
            return Err(OwnerError::Pending);
        }
        if self.rejection_pending {
            return self.deliver_rejection(registry);
        }
        let event = self
            .pump
            .next(registry, timeout)
            .map_err(OwnerError::Pump)?;
        match event {
            PumpEvent::Rejected(_) => {
                self.rejection_pending = true;
                self.deliver_rejection(registry)
            }
            PumpEvent::Accepted(step @ Step::Advanced { .. })
            | PumpEvent::Outcome(ObservationProgress::Advanced(step @ Step::Advanced { .. })) => {
                self.pending = Some(step);
                Ok(OwnerEvent::StepReady)
            }
            other => Ok(OwnerEvent::Other(other)),
        }
    }

    fn deliver_rejection(
        &mut self,
        registry: &OperationRegistry,
    ) -> Result<OwnerEvent, OwnerError> {
        let context = self
            .pump
            .context()
            .ok_or(OwnerError::Pump(PumpError::Failed))?;
        if context.operation != self.operation {
            return Err(OwnerError::Pump(PumpError::Failed));
        }
        let step = registry
            .apply_verification_rejection(&context)
            .map_err(|error| OwnerError::Pump(PumpError::Delivery(error)))?;
        self.rejection_pending = false;
        match step {
            step @ Step::Advanced { .. } => {
                self.pending = Some(step);
                Ok(OwnerEvent::StepReady)
            }
            _ => Err(OwnerError::Pump(PumpError::Failed)),
        }
    }

    /// If the transfer is refused, the step remains with this owner.
    pub(crate) fn handoff(&mut self, effects: &EffectOwner) -> Result<(), OwnerError> {
        let step = self.pending.as_ref().ok_or(OwnerError::NoStep)?;
        effects
            .accept_step(self.operation, step)
            .map_err(OwnerError::Handoff)?;
        self.pending = None;
        Ok(())
    }

    /// It does no more than one reading of the channel. If the transmission fails, the next
    /// call will first repeat the transmission of the retained step without reading the
    /// channel.
    /// Term and recall supervision should work independently of this call; calling retains
    /// owners and disassembles unsupported effects.
    pub(crate) async fn advance<T: Future<Output = ()>>(
        &mut self,
        registry: &OperationRegistry,
        effects: &EffectOwner,
        scheduler: &std::sync::Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        read_timeout: Duration,
        cleanup_timeout: impl Fn() -> T,
    ) -> Result<AdvanceOutcome, AdvanceError> {
        if !self.needs_drive {
            if self.pending.is_none() {
                match self
                    .next(registry, read_timeout)
                    .map_err(AdvanceError::Owner)?
                {
                    OwnerEvent::StepReady => {}
                    OwnerEvent::Other(event) => return Ok(AdvanceOutcome::Other(event)),
                }
            }
            self.handoff(effects).map_err(AdvanceError::Owner)?;
            self.needs_drive = true;
        }
        let outcome = effects
            .drive_negative_scheduled(registry, self.operation, scheduler, broker, cleanup_timeout)
            .await
            .map_err(AdvanceError::Drive)?;
        // The external effect still belongs to the performer, and as long as his step is
        // held, the next reading should not overtake the cancellation or the cleaning.
        self.needs_drive = matches!(outcome, DriveOutcome::ExternalEffect)
            && effects.step(self.operation).is_some();
        Ok(AdvanceOutcome::Driven(outcome))
    }

    /// Accepts a limited number of events within the total term.
    /// Progress events and unsupported effects return to the caller; recall and cleaning
    /// oversight remains independent.
    pub(crate) async fn drive_until_blocked<T: Future<Output = ()>>(
        &mut self,
        registry: &OperationRegistry,
        effects: &EffectOwner,
        scheduler: &std::sync::Mutex<carosign_runtime::scheduler::Scheduler>,
        broker: &dyn crate::resources::TrustedCleanupBroker,
        read_budget: Duration,
        cleanup_timeout: impl Fn() -> T,
    ) -> Result<AdvanceOutcome, AdvanceError> {
        let started = std::time::Instant::now();
        for _ in 0..8 {
            let remaining = read_budget.saturating_sub(started.elapsed());
            let outcome = self
                .advance(
                    registry,
                    effects,
                    scheduler,
                    broker,
                    remaining,
                    &cleanup_timeout,
                )
                .await?;
            match outcome {
                AdvanceOutcome::Driven(DriveOutcome::ExternalEffect)
                    if effects.step(self.operation).is_none() =>
                {
                    continue;
                }
                other => return Ok(other),
            }
        }
        Ok(AdvanceOutcome::Driven(DriveOutcome::StepLimit))
    }
}
