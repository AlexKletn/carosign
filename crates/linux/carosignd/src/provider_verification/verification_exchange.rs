use super::{
    ActorVerificationError, ActorVerificationMessage, BindingKind, CommandId, DeviceEpoch,
    Duration, Event, OperationId, PermittedSet, ProviderFrame, ProviderTransport, ReceivedFrame,
    Session, VerificationError, VerificationMessage, VerificationPhase, VerificationTransport,
    WorkerInstanceId, events,
};

/// The only reader for one check.
pub struct VerificationExchange {
    pub(super) transport: VerificationTransport,
    pub(in super::super) session: Session,
    pub(in super::super) command: CommandId,
    pub(in super::super) authenticator: carosign_core::ids::AuthenticatorId,
    pub(super) permitted: PermittedSet,
    phase: VerificationPhase,
}

impl VerificationExchange {
    /// Checks the full command and sends it exactly once.
    /// Admitting and selecting a trusted channel remains the caller's responsibility.
    pub fn start(
        transport: ProviderTransport,
        frame: ProviderFrame,
        permitted: PermittedSet,
        timeout: Duration,
    ) -> Result<Self, VerificationError> {
        Self::start_channel(
            VerificationTransport::Direct(transport),
            frame,
            permitted,
            timeout,
        )
    }

    /// Starts the check through a held tunnel broker.
    pub fn start_tunnel(
        transport: carosign_linux_ipc::provider::tunnel::TunnelClient,
        frame: ProviderFrame,
        permitted: PermittedSet,
        timeout: Duration,
    ) -> Result<Self, VerificationError> {
        Self::start_channel(
            VerificationTransport::Tunnel(Box::new(transport)),
            frame,
            permitted,
            timeout,
        )
    }

    pub(crate) fn start_channel(
        transport: VerificationTransport,
        frame: ProviderFrame,
        permitted: PermittedSet,
        timeout: Duration,
    ) -> Result<Self, VerificationError> {
        Self::start_channel_inner(transport, frame, permitted, timeout)
    }

    fn start_channel_inner(
        mut transport: VerificationTransport,
        frame: ProviderFrame,
        permitted: PermittedSet,
        timeout: Duration,
    ) -> Result<Self, VerificationError> {
        let carosign_provider_sdk::command::Command::StartVerification {
            permitted_enrollments,
            authenticator,
            ..
        } = carosign_provider_sdk::command::parse(&frame)
            .map_err(|_| VerificationError::Protocol)?
        else {
            return Err(VerificationError::Protocol);
        };
        if permitted.bindings().iter().any(|binding| binding.kind == BindingKind::ScopedVault)
            || permitted_enrollments.len() != permitted.len()
            || permitted_enrollments.iter().zip(permitted.bindings()).any(|(wire, binding)| {
                let Some(reference) = &wire.enrollment else { return true; };
                reference.enrollment_id.as_slice() != binding.enrollment.id.as_bytes()
                    || reference.authenticator_id.as_slice() != binding.enrollment.authenticator.as_bytes()
                    || reference.generation != binding.enrollment.generation.value()
                    || wire.kind != match binding.kind {
                        BindingKind::ProviderOpaque => carosign_protocol::carosign::provider::v1::EnrollmentBindingKind::EnrollmentBindingProviderOpaque as i32,
                        BindingKind::ScopedVault => carosign_protocol::carosign::provider::v1::EnrollmentBindingKind::EnrollmentBindingScopedVault as i32,
                    }
                    || wire.provider_opaque_ref != binding.provider_ref
                    || wire.format_id != binding.format_id.as_deref().unwrap_or("")
                    || wire.format_version != binding.format_version
            })
        {
            return Err(VerificationError::Protocol);
        }
        let worker = WorkerInstanceId::try_from(frame.worker_instance_id.as_slice())
            .map_err(|_| VerificationError::Protocol)?;
        let command = CommandId::try_from(frame.command_id.as_slice())
            .map_err(|_| VerificationError::Protocol)?;
        let operation = OperationId::try_from(frame.operation_id.as_slice())
            .map_err(|_| VerificationError::Protocol)?;
        let epoch =
            DeviceEpoch::new(frame.device_epoch).map_err(|_| VerificationError::Protocol)?;
        let authenticator = carosign_core::ids::AuthenticatorId::try_from(
            authenticator.authenticator_id.as_slice(),
        )
        .map_err(|_| VerificationError::Protocol)?;
        let session = Session {
            worker: Some(worker),
            device_epoch: Some(epoch),
            operation: Some(operation),
            ..Session::default()
        };
        transport
            .send(frame, timeout)
            .map_err(VerificationError::Transport)?;
        Ok(Self {
            transport,
            session,
            command,
            authenticator,
            permitted,
            phase: VerificationPhase::AwaitingAck,
        })
    }

    /// Current stage.
    pub fn phase(&self) -> VerificationPhase {
        self.phase
    }

    /// Reads exactly one event; a violation closes the channel.
    pub fn next(&mut self, timeout: Duration) -> Result<VerificationMessage, VerificationError> {
        if matches!(
            self.phase,
            VerificationPhase::Finished | VerificationPhase::Failed
        ) {
            return Err(VerificationError::Finished);
        }
        let result = self.read_next(timeout);
        if result.is_err() {
            self.phase = VerificationPhase::Failed;
            self.transport.abort();
        }
        result
    }

    /// Transmits one session response to the appropriate actor.
    /// The caller still performs the actor's steps and cleaning effects.
    pub fn next_for_actor(
        &mut self,
        actor: &mut carosign_runtime::OperationActor,
        timeout: Duration,
    ) -> Result<ActorVerificationMessage, ActorVerificationError> {
        use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, OperationPhase};
        use carosign_runtime::OperationMsg;
        let state = actor.state();
        if Some(state.common.context.operation_id) != self.session.operation
            || state.common.context.authenticator.id != self.authenticator
            || Some(state.common.context.authenticator.device_epoch) != self.session.device_epoch
            || state.common.worker.is_none_or(|worker| {
                Some(worker.instance) != self.session.worker
                    || Some(worker.device_epoch) != self.session.device_epoch
            })
            || state
                .common
                .command
                .is_none_or(|command| command.id != self.command)
        {
            return Err(ActorVerificationError::ContextMismatch);
        }
        let pending = match self.phase {
            VerificationPhase::AwaitingAck => Some(
                *actor
                    .pending()
                    .iter()
                    .find(|effect| effect.spec == EffectSpec::SendVerify)
                    .ok_or(ActorVerificationError::ContextMismatch)?,
            ),
            VerificationPhase::Running if state.common.phase == OperationPhase::Running => None,
            _ => return Err(ActorVerificationError::ContextMismatch),
        };
        match self
            .next(timeout)
            .map_err(ActorVerificationError::Exchange)?
        {
            VerificationMessage::Accepted => {
                let effect = pending.ok_or(ActorVerificationError::ContextMismatch)?;
                actor
                    .post(OperationMsg::Domain(DomainInput::Receipt {
                        effect,
                        receipt: EffectReceipt::SendAccepted,
                    }))
                    .map_err(|_| ActorVerificationError::MailboxFull)?;
                Ok(ActorVerificationMessage::Accepted)
            }
            VerificationMessage::Rejected(error) => Ok(ActorVerificationMessage::Rejected(error)),
            VerificationMessage::Progress(progress) => {
                Ok(ActorVerificationMessage::Progress(progress))
            }
            VerificationMessage::Outcome { observation } => {
                actor
                    .hold_verification_observation(*observation)
                    .map_err(ActorVerificationError::Observation)?;
                match actor.advance_verification_observation() {
                    Ok(()) => Ok(ActorVerificationMessage::OutcomeQueued),
                    Err(error) => Ok(ActorVerificationMessage::OutcomeHeld(error)),
                }
            }
        }
    }

    fn read_next(&mut self, timeout: Duration) -> Result<VerificationMessage, VerificationError> {
        let frame = ReceivedFrame(
            self.transport
                .receive(timeout)
                .map_err(VerificationError::Transport)?,
        );
        let frame = &frame.0;
        if frame.worker_instance_id.as_slice()
            != self
                .session
                .worker
                .ok_or(VerificationError::Protocol)?
                .as_bytes()
            || frame.device_epoch
                != self
                    .session
                    .device_epoch
                    .ok_or(VerificationError::Protocol)?
                    .value()
            || frame.operation_id.as_slice()
                != self
                    .session
                    .operation
                    .ok_or(VerificationError::Protocol)?
                    .as_bytes()
            || frame.command_id.as_slice() != self.command.as_bytes()
        {
            return Err(VerificationError::Protocol);
        }
        let event = events::parse(frame).map_err(|_| VerificationError::Protocol)?;
        match (self.phase, event) {
            (
                VerificationPhase::AwaitingAck,
                Event::Accepted {
                    rejection: None, ..
                },
            ) => {
                self.session.accept(self.command);
                self.phase = VerificationPhase::Running;
                Ok(VerificationMessage::Accepted)
            }
            (
                VerificationPhase::AwaitingAck,
                Event::Accepted {
                    rejection: Some(error),
                    ..
                },
            ) => {
                self.phase = VerificationPhase::Finished;
                Ok(VerificationMessage::Rejected(error))
            }
            (
                VerificationPhase::Running,
                Event::Progress {
                    feedback: Some(_), ..
                }
                | Event::Progress {
                    capture_state: Some(_),
                    ..
                },
            ) if matches!(self.transport, VerificationTransport::Direct(_)) => {
                Err(VerificationError::Protocol)
            }
            (VerificationPhase::Running, progress @ Event::Progress { .. }) => {
                let progress = self
                    .session
                    .accept_progress_event(progress)
                    .map_err(|_| VerificationError::Protocol)?;
                Ok(VerificationMessage::Progress(progress))
            }
            (VerificationPhase::Running, outcome @ Event::Verification { .. }) => {
                let observation = self
                    .session
                    .accept_verification_outcome(outcome, &self.permitted)
                    .map_err(|_| VerificationError::Protocol)?;
                self.phase = VerificationPhase::Finished;
                Ok(VerificationMessage::Outcome {
                    observation: Box::new(observation),
                })
            }
            _ => Err(VerificationError::Protocol),
        }
    }

    /// Returns the channel after a normal result or failure.
    pub fn into_transport(self) -> Result<ProviderTransport, VerificationError> {
        match self.into_channel()? {
            VerificationTransport::Direct(transport) => Ok(transport),
            VerificationTransport::Tunnel(_) => Err(VerificationError::Protocol),
        }
    }

    /// Returns the channel with a preserved state after a normal result.
    pub fn into_channel(self) -> Result<VerificationTransport, VerificationError> {
        if self.phase != VerificationPhase::Finished {
            return Err(VerificationError::Protocol);
        }
        Ok(self.transport)
    }
}
