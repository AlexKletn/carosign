use super::{EnrollmentOutcome, ProgressObservation, SessionError, VerificationObservation};
use crate::operation::events::{CapturePercent, Progress};
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{CommandId, MutationId, OperationId, WorkerInstanceId};

/// Session status.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub worker: Option<WorkerInstanceId>,
    /// The age of the tethered device.
    pub device_epoch: Option<DeviceEpoch>,
    /// Active surgery.
    pub operation: Option<OperationId>,
    /// Accepted command.
    pub accepted: Option<CommandId>,
    /// The result has already been heard.
    pub terminal: bool,
    /// The last accepted progress number of this operation.
    pub last_progress_sequence: Option<u64>,
}

impl Session {
    /// Checks the context of the outcome and optional enrollment.
    /// The answer remains the worker’s statement and is not evidence.
    pub fn accept_verification_outcome(
        &mut self,
        event: carosign_provider_sdk::events::Event,
        permitted: &crate::operation::bindings::PermittedSet,
    ) -> Result<VerificationObservation, SessionError> {
        use carosign_provider_sdk::events::Event;
        let Event::Verification {
            worker,
            device_epoch,
            command,
            operation,
            matched_enrollment,
            status,
            evidence,
            format_id,
            ..
        } = &event
        else {
            return Err(SessionError::InvalidVerificationOutcome);
        };
        use carosign_provider_sdk::events::ProviderEvidence;
        use carosign_provider_sdk::events::VerificationStatus;
        if *status == VerificationStatus::VerifyStatusUnspecified
            || (*status != VerificationStatus::VerifyMatched
                && (evidence.is_some() || matched_enrollment.is_some()))
            || format_id.len() > 128
            || match evidence {
                Some(ProviderEvidence::Reference(value)) => {
                    value.len() != 16 || value.iter().all(|byte| *byte == 0)
                }
                Some(ProviderEvidence::OpaqueToken(value)) => value.len() > 16_384,
                None => false,
            }
        {
            return Err(SessionError::InvalidVerificationOutcome);
        }
        let worker = WorkerInstanceId::try_from(worker.as_slice())
            .map_err(|_| SessionError::InvalidVerificationOutcome)?;
        let epoch = DeviceEpoch::new(*device_epoch)
            .map_err(|_| SessionError::InvalidVerificationOutcome)?;
        let command = CommandId::try_from(command.as_slice())
            .map_err(|_| SessionError::InvalidVerificationOutcome)?;
        let operation = OperationId::try_from(operation.as_slice())
            .map_err(|_| SessionError::InvalidVerificationOutcome)?;
        if self.worker != Some(worker)
            || self.device_epoch != Some(epoch)
            || self.operation != Some(operation)
        {
            return Err(SessionError::InvalidVerificationOutcome);
        }
        if self.terminal {
            return Err(SessionError::SecondTerminal);
        }
        match self.accepted {
            None => return Err(SessionError::NotAccepted),
            Some(expected) if expected != command => return Err(SessionError::WrongCommand),
            Some(_) => {}
        }
        if let Some(reference) = matched_enrollment {
            let valid = permitted.bindings().iter().any(|binding| {
                binding.enrollment.id.as_bytes().as_slice() == reference.enrollment_id.as_slice()
                    && binding.enrollment.authenticator.as_bytes().as_slice()
                        == reference.authenticator_id.as_slice()
                    && binding.enrollment.generation.value() == reference.generation
            });
            if !valid {
                return Err(SessionError::InvalidVerificationOutcome);
            }
        }
        self.terminal = true;
        Ok(VerificationObservation {
            event,
            permitted: permitted.clone(),
        })
    }

    /// Verifies the outcome of the registration with the session and the plan before
    /// transferring to the owner.
    ///
    /// The frame must pre-check the channel and SDK. The correct echo does not prove a
    /// physical change and does not allow the registry to be recorded.
    pub fn accept_enrollment_outcome(
        &mut self,
        event: carosign_provider_sdk::events::Event,
        expected_mutation: MutationId,
        planned: carosign_core::enrollment::EnrollmentRef,
    ) -> Result<EnrollmentOutcome, SessionError> {
        use carosign_provider_sdk::events::{Event, MutationStatus};
        let Event::Mutation {
            worker,
            device_epoch,
            command,
            operation,
            mutation,
            status,
            enrollment,
            recovery_ref,
            provider_ref,
            ..
        } = &event
        else {
            return Err(SessionError::InvalidMutationOutcome);
        };
        let worker = WorkerInstanceId::try_from(worker.as_slice())
            .map_err(|_| SessionError::InvalidMutationOutcome)?;
        let epoch =
            DeviceEpoch::new(*device_epoch).map_err(|_| SessionError::InvalidMutationOutcome)?;
        let command = CommandId::try_from(command.as_slice())
            .map_err(|_| SessionError::InvalidMutationOutcome)?;
        let operation = OperationId::try_from(operation.as_slice())
            .map_err(|_| SessionError::InvalidMutationOutcome)?;
        let mutation = MutationId::try_from(mutation.as_slice())
            .map_err(|_| SessionError::InvalidMutationOutcome)?;
        if self.worker != Some(worker)
            || self.device_epoch != Some(epoch)
            || self.operation != Some(operation)
            || mutation != expected_mutation
        {
            return Err(SessionError::InvalidMutationOutcome);
        }
        if self.terminal {
            return Err(SessionError::SecondTerminal);
        }
        match self.accepted {
            None => return Err(SessionError::NotAccepted),
            Some(accepted) if accepted != command => return Err(SessionError::WrongCommand),
            Some(_) => {}
        }
        if let Some(echo) = enrollment {
            if echo.enrollment_id.as_slice() != planned.id.as_bytes()
                || echo.authenticator_id.as_slice() != planned.authenticator.as_bytes()
                || echo.generation != planned.generation.value()
            {
                return Err(SessionError::InvalidMutationOutcome);
            }
        } else if *status == MutationStatus::MutationProviderCompleted {
            return Err(SessionError::InvalidMutationOutcome);
        }
        if *status == MutationStatus::Unspecified
            || recovery_ref.len() > carosign_core::enrollment::OPAQUE_REFERENCE_MAX
            || provider_ref.len() > carosign_core::enrollment::OPAQUE_REFERENCE_MAX
        {
            return Err(SessionError::InvalidMutationOutcome);
        }
        self.terminal = true;
        Ok(EnrollmentOutcome {
            event,
            operation,
            mutation,
            planned,
        })
    }

    /// Checks the disassembled SDK event, saving its contents for delivery.
    ///
    /// The frame must first pass channel sequence check and SDK parsing. The error here
    /// requires session processing as a protocol violation; it does not prove that the device
    /// is at rest.
    pub fn accept_progress_event(
        &mut self,
        event: carosign_provider_sdk::events::Event,
    ) -> Result<ProgressObservation, SessionError> {
        use carosign_provider_sdk::events::{Event, ProgressKind};
        let Event::Progress {
            worker,
            device_epoch,
            command,
            operation,
            sequence,
            kind,
            percent,
            feedback,
            capture_state,
        } = &event
        else {
            return Err(SessionError::MalformedProgress);
        };
        let worker = WorkerInstanceId::try_from(worker.as_slice())
            .map_err(|_| SessionError::MalformedProgress)?;
        let epoch = DeviceEpoch::new(*device_epoch).map_err(|_| SessionError::MalformedProgress)?;
        let command =
            CommandId::try_from(command.as_slice()).map_err(|_| SessionError::MalformedProgress)?;
        let operation = OperationId::try_from(operation.as_slice())
            .map_err(|_| SessionError::MalformedProgress)?;
        let progress = if let Some(state) = capture_state {
            Progress::CaptureState(*state)
        } else {
            match (kind, *percent, *feedback) {
                (ProgressKind::ProgressUnspecified, 0, Some(feedback))
                    if carosign_protocol::validation::check_capture_feedback(&feedback) =>
                {
                    Progress::Feedback(feedback)
                }
                (ProgressKind::ProgressPlaceFinger, 0, None) => Progress::PlaceFinger,
                (ProgressKind::ProgressMoveFinger, 0, None) => Progress::MoveFinger,
                (ProgressKind::ProgressRemoveFinger, 0, None) => Progress::RemoveFinger,
                (ProgressKind::ProgressRetry, 0, None) => Progress::Retry,
                (ProgressKind::ProgressWorking, 0, None) => Progress::Working,
                (ProgressKind::ProgressCapturing, percent, None) => Progress::Capturing(
                    CapturePercent::new(percent).ok_or(SessionError::MalformedProgress)?,
                ),
                _ => return Err(SessionError::MalformedProgress),
            }
        };
        self.accept_progress(worker, epoch, operation, command, *sequence)?;
        Ok(ProgressObservation {
            operation,
            provider_sequence: *sequence,
            progress,
        })
    }

    /// Checks the binding and stroke number before transmitting the observation of the
    /// operation.
    ///
    /// If you refuse, the state of the session does not change, gluing when delivered to the
    /// customer does not affect the sequence of the worker's messages.
    pub fn accept_progress(
        &mut self,
        worker: WorkerInstanceId,
        epoch: DeviceEpoch,
        operation: OperationId,
        command: CommandId,
        sequence: u64,
    ) -> Result<(), SessionError> {
        if self.worker != Some(worker)
            || self.device_epoch != Some(epoch)
            || self.operation != Some(operation)
        {
            return Err(SessionError::StaleProgress);
        }
        if self.terminal {
            return Err(SessionError::ProgressAfterTerminal);
        }
        match self.accepted {
            None => return Err(SessionError::NotAccepted),
            Some(accepted) if accepted != command => return Err(SessionError::WrongCommand),
            Some(_) => {}
        }
        let valid = match self.last_progress_sequence {
            None => sequence == 1,
            Some(previous) => sequence > previous,
        };
        if !valid {
            return Err(SessionError::ProgressOutOfOrder);
        }
        self.last_progress_sequence = Some(sequence);
        Ok(())
    }

    /// Celebrates the first acceptance of the command.
    ///
    /// Re-confirmation should not replace an already accepted command.
    /// Reporting a violation of protocol requires a check at the session boundary.
    pub fn accept(&mut self, command: CommandId) {
        if !self.terminal {
            self.accepted.get_or_insert(command);
        }
    }

    /// Celebrates the outcome.
    ///
    /// ⚠️ The second result of the same operation is a breakdown, not a “getting better”.
    pub fn terminal(&mut self) -> Result<(), SessionError> {
        if self.terminal {
            return Err(SessionError::SecondTerminal);
        }
        if self.accepted.is_none() {
            return Err(SessionError::NotAccepted);
        }
        self.terminal = true;
        Ok(())
    }

    /// Can I start a new operation in this session?
    pub fn free(&self) -> bool {
        self.operation.is_none() && self.accepted.is_none() && !self.terminal
    }
}

#[path = "session.tests.rs"]
#[cfg(test)]
mod tests;
