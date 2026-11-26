use super::{
    DriveOutcome, Duration, Future, OperationRegistry, StartError, StartStage, VerificationOwner,
    VerificationPump, VerificationStartPorts,
};
#[cfg(test)]
use carosign_runtime::operation::actor::Step;

/// Tie the command and transfer its step to a single attempt to send.
/// The owner must be retained until the handoff is completed; it does not establish trust in
/// the channel, the broker, or the devolved source of authority.
pub(crate) struct VerificationStart {
    context: carosign_adapters::provider::VerificationFrameContext,
    stage: StartStage,
}

impl VerificationStart {
    #[cfg(test)]
    pub(crate) fn pending_step(&self) -> Option<&Step> {
        match &self.stage {
            StartStage::Bound(step) => Some(step),
            _ => None,
        }
    }
    pub(crate) fn bind(
        registry: &OperationRegistry,
        context: carosign_adapters::provider::VerificationFrameContext,
    ) -> Result<Self, StartError> {
        let step = registry
            .retain_verification_binding(&context)
            .map_err(StartError::Operation)?;
        Ok(Self {
            context,
            stage: StartStage::Bound(step),
        })
    }

    /// Resumes an untransferred step after the loss of its owner.
    pub(crate) fn resume(
        registry: &OperationRegistry,
        context: carosign_adapters::provider::VerificationFrameContext,
    ) -> Result<Self, StartError> {
        let step = registry
            .retained_verification_binding(&context)
            .map_err(StartError::Operation)?;
        Ok(Self {
            context,
            stage: StartStage::Bound(step),
        })
    }

    /// Transmits the step before re-checking the policy and a single shipment.
    /// When the owner of the effects is busy, the step remains inside the object.
    pub(crate) async fn start<T: Future<Output = ()>>(
        &mut self,
        ports: &VerificationStartPorts<'_>,
        transport: carosign_linux_ipc::provider::transport::ProviderTransport,
        send_timeout: Duration,
        cleanup_timeout: impl Fn() -> T,
    ) -> Result<VerificationOwner, StartError> {
        self.start_channel(
            ports,
            crate::provider_verification::VerificationTransport::Direct(transport),
            send_timeout,
            cleanup_timeout,
        )
        .await
    }

    pub(crate) async fn start_channel<T: Future<Output = ()>>(
        &mut self,
        ports: &VerificationStartPorts<'_>,
        transport: crate::provider_verification::VerificationTransport,
        send_timeout: Duration,
        cleanup_timeout: impl Fn() -> T,
    ) -> Result<VerificationOwner, StartError> {
        if matches!(self.stage, StartStage::Bound(_)) {
            ports
                .registry
                .handoff_verification_binding(&self.context, ports.effects)
                .map_err(|error| match error {
                    crate::operations::VerificationBindingFailure::Operation(error) => {
                        StartError::Operation(error)
                    }
                    crate::operations::VerificationBindingFailure::Handoff(error) => {
                        StartError::Handoff(error)
                    }
                })?;
            self.stage = StartStage::HandedOff;
        }
        if matches!(self.stage, StartStage::Finished) {
            return Err(StartError::Finished);
        }
        let outcome = ports
            .effects
            .drive_negative_scheduled(
                ports.registry,
                self.context.operation,
                ports.scheduler,
                ports.broker,
                cleanup_timeout,
            )
            .await
            .map_err(StartError::Drive)?;
        if !matches!(outcome, DriveOutcome::ExternalEffect)
            || ports.effects.step(self.context.operation).is_some()
        {
            return Err(StartError::OutstandingEffects);
        }
        let attempt = ports
            .registry
            .begin_revalidated_verification_send(&self.context, ports.clock, ports.authority)
            .await;
        self.stage = StartStage::Finished;
        let attempt = attempt.map_err(|error| match error {
            crate::operations::VerificationPreparationFailure::Operation(error) => {
                StartError::Operation(error)
            }
            other => StartError::Preparation(other),
        })?;
        let exchange = attempt
            .send_channel(transport, send_timeout)
            .map_err(StartError::Send)?;
        Ok(VerificationOwner::new(
            self.context.operation,
            VerificationPump::new(exchange),
        ))
    }
}
