use super::{ScopedProviderAssertion, SessionError};
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{CommandId, OperationId, WorkerInstanceId};

/// Verification response matched by the session to a specific set of registrations.
/// This is the provider's statement; the eligibility and credibility are verified separately.
#[derive(Debug, PartialEq)]
pub struct VerificationObservation {
    pub(super) event: carosign_provider_sdk::events::Event,
    pub(super) permitted: crate::operation::bindings::PermittedSet,
}

impl VerificationObservation {
    pub(crate) fn event_mut(&mut self) -> &mut carosign_provider_sdk::events::Event {
        &mut self.event
    }
    /// Initial statement with context and type of material.
    pub fn event(&self) -> &carosign_provider_sdk::events::Event {
        &self.event
    }
    /// A set for which a match has been verified.
    pub fn permitted(&self) -> &crate::operation::bindings::PermittedSet {
        &self.permitted
    }

    /// It separates a single statement without material from a session-tested response, it's
    /// just a correlation with the command and the set, not an authority, not a confirmation
    /// of the origin of the process or the result.
    pub fn into_scoped_assertion(self) -> Result<ScopedProviderAssertion, SessionError> {
        use carosign_provider_sdk::events::{Event, VerificationStatus};

        let Event::Verification {
            worker,
            device_epoch,
            command,
            operation,
            status: VerificationStatus::VerifyMatched,
            evidence: None,
            matched_enrollment,
            error: None,
            ..
        } = self.event
        else {
            return Err(SessionError::InvalidVerificationOutcome);
        };
        let matched = match matched_enrollment {
            Some(reference) => Some(
                self.permitted
                    .bindings()
                    .iter()
                    .find(|binding| {
                        binding.enrollment.id.as_bytes().as_slice() == reference.enrollment_id
                            && binding.enrollment.authenticator.as_bytes().as_slice()
                                == reference.authenticator_id
                            && binding.enrollment.generation.value() == reference.generation
                    })
                    .map(|binding| binding.enrollment)
                    .ok_or(SessionError::InvalidVerificationOutcome)?,
            ),
            None => None,
        };
        Ok(ScopedProviderAssertion {
            worker: WorkerInstanceId::try_from(worker.as_slice())
                .map_err(|_| SessionError::InvalidVerificationOutcome)?,
            epoch: DeviceEpoch::new(device_epoch)
                .map_err(|_| SessionError::InvalidVerificationOutcome)?,
            command: CommandId::try_from(command.as_slice())
                .map_err(|_| SessionError::InvalidVerificationOutcome)?,
            operation: OperationId::try_from(operation.as_slice())
                .map_err(|_| SessionError::InvalidVerificationOutcome)?,
            permitted: self.permitted,
            matched,
        })
    }
}
