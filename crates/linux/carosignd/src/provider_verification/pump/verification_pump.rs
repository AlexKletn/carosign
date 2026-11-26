use super::{
    Duration, PendingDelivery, ProviderTransport, PumpError, PumpEvent, VerificationContext,
    VerificationError, VerificationExchange, VerificationMessage, VerificationTransport,
};

/// The session owner holds the result until handed over to the relevant actor.
/// Does not confirm trusted launch, authority or cleaning.
pub struct VerificationPump {
    exchange: VerificationExchange,
    pending: Option<PendingDelivery>,
    failed: bool,
}

impl VerificationPump {
    pub(crate) fn context(&self) -> Option<VerificationContext> {
        Some(VerificationContext {
            operation: self.exchange.session.operation?,
            worker: self.exchange.session.worker?,
            command: self.exchange.command,
            authenticator: self.exchange.authenticator,
            epoch: self.exchange.session.device_epoch?,
        })
    }

    /// Accepts an already open exchange; the sending is not repeated.
    pub fn new(exchange: VerificationExchange) -> Self {
        Self {
            exchange,
            pending: None,
            failed: false,
        }
    }

    /// Reads one event or repeats delivery of the withheld result.
    /// The channel is read without blocking the table of operations.
    pub fn next(
        &mut self,
        registry: &crate::operations::OperationRegistry,
        timeout: Duration,
    ) -> Result<PumpEvent, PumpError> {
        if self.failed {
            return Err(PumpError::Failed);
        }
        if self.pending.is_none() {
            match self.exchange.next(timeout).map_err(PumpError::Exchange)? {
                VerificationMessage::Accepted => self.pending = Some(PendingDelivery::Accepted),
                VerificationMessage::Rejected(error) => return Ok(PumpEvent::Rejected(error)),
                VerificationMessage::Progress(progress) => {
                    if matches!(
                        progress.progress,
                        carosign_runtime::operation::events::Progress::Feedback(_)
                            | carosign_runtime::operation::events::Progress::CaptureState(_)
                    ) {
                        registry
                            .publish_provider_progress(progress)
                            .map_err(PumpError::Delivery)?;
                    }
                    return Ok(PumpEvent::Progress(progress));
                }
                VerificationMessage::Outcome { observation } => {
                    self.pending = Some(PendingDelivery::Outcome(observation))
                }
            }
        }
        let operation = self.exchange.session.operation.ok_or(PumpError::Failed)?;
        if matches!(
            self.pending,
            Some(PendingDelivery::Accepted | PendingDelivery::AckQueued)
        ) {
            let context = VerificationContext {
                operation,
                worker: self.exchange.session.worker.ok_or(PumpError::Failed)?,
                command: self.exchange.command,
                authenticator: self.exchange.authenticator,
                epoch: self
                    .exchange
                    .session
                    .device_epoch
                    .ok_or(PumpError::Failed)?,
            };
            if matches!(self.pending, Some(PendingDelivery::AckQueued)) {
                return match registry.verification_ack_processed(&context) {
                    Ok(()) => {
                        self.pending = None;
                        self.next(registry, timeout)
                    }
                    Err(error) => {
                        if matches!(
                            error,
                            crate::operations::ObservationFailure::ContextMismatch
                        ) {
                            self.failed = true;
                        }
                        Err(PumpError::Delivery(error))
                    }
                };
            }
            return match registry.apply_verification_ack(&context) {
                Ok(step) => {
                    self.pending = Some(PendingDelivery::AckQueued);
                    Ok(PumpEvent::Accepted(step))
                }
                Err(error) => {
                    if matches!(
                        error,
                        crate::operations::ObservationFailure::ContextMismatch
                            | crate::operations::ObservationFailure::Rejected(_)
                    ) {
                        self.failed = true;
                    }
                    Err(PumpError::Delivery(error))
                }
            };
        }
        let Some(PendingDelivery::Outcome(observation)) = self.pending.take() else {
            return Err(PumpError::Failed);
        };
        match registry.apply_verification_observation(operation, *observation) {
            Ok(progress) => Ok(PumpEvent::Outcome(progress)),
            Err((error, observation)) => {
                self.pending = observation.map(PendingDelivery::Outcome);
                if self.pending.is_none() {
                    self.failed = true;
                }
                Err(PumpError::Delivery(error))
            }
        }
    }

    /// Returns the channel only after delivery of the result or failure of the command.
    pub fn into_transport(self) -> Result<ProviderTransport, VerificationError> {
        if self.pending.is_some() || self.failed {
            return Err(VerificationError::Protocol);
        }
        self.exchange.into_transport()
    }

    /// Returns a direct channel or tunnel after delivery is complete.
    pub fn into_channel(self) -> Result<VerificationTransport, VerificationError> {
        if self.pending.is_some() || self.failed {
            return Err(VerificationError::Protocol);
        }
        self.exchange.into_channel()
    }
}
