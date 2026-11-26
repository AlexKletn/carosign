use super::SessionError;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{CommandId, OperationId, WorkerInstanceId};

/// A one-time moveable statement, verified only with the session.
///
/// It is neither proof of origin, nor an assessment, nor a right of entry; the examiner must
/// independently establish the trust and the exact area.
#[derive(Debug)]
pub struct ScopedProviderAssertion {
    pub(super) worker: WorkerInstanceId,
    pub(super) epoch: DeviceEpoch,
    pub(super) command: CommandId,
    pub(super) operation: OperationId,
    pub(super) permitted: crate::operation::bindings::PermittedSet,
    pub(super) matched: Option<carosign_core::enrollment::EnrollmentRef>,
}

impl ScopedProviderAssertion {
    /// Moves the correlated statement to the portable evidence record.
    /// The conversion does not certify the caller, session or recipient; it is the
    /// responsibility of the independent owner to do so before a positive evaluation.
    pub fn into_evidence_record(
        self,
        context: &crate::ports::ScopedAssertionContext,
    ) -> Result<carosign_evidence::store::ProviderAssertionRecord, SessionError> {
        if context.authenticator.device_epoch != self.epoch
            || self
                .permitted
                .bindings()
                .iter()
                .any(|binding| binding.enrollment.authenticator != context.authenticator.id)
        {
            return Err(SessionError::InvalidVerificationOutcome);
        }
        Ok(carosign_evidence::store::ProviderAssertionRecord {
            operation: self.operation,
            worker: self.worker,
            command: self.command,
            subject: context.subject,
            authenticator: context.authenticator,
            purpose: context.purpose,
            recipient: context.recipient,
            challenge: context.challenge,
            deadline: context.deadline,
            permitted_digest: self.permitted.scoped_digest(),
            permitted: self
                .permitted
                .bindings()
                .iter()
                .map(|binding| binding.enrollment)
                .collect(),
            matched: self.matched,
        })
    }

    /// The worker with whom the session checked the answer.
    pub fn worker(&self) -> WorkerInstanceId {
        self.worker
    }
    /// The age of the device.
    pub fn epoch(&self) -> DeviceEpoch {
        self.epoch
    }
    /// Check-in command.
    pub fn command(&self) -> CommandId {
        self.command
    }
    /// Check-in operation.
    pub fn operation(&self) -> OperationId {
        self.operation
    }
    /// A complete limited set transmitted to the session.
    pub fn permitted(&self) -> &crate::operation::bindings::PermittedSet {
        &self.permitted
    }
    /// Optional registration; if available, it is included in the set.
    pub fn matched(&self) -> Option<carosign_core::enrollment::EnrollmentRef> {
        self.matched
    }
}
