use crate::recipient::ChallengeRef;
use crate::recipient::RecipientRef;
use carosign_core::device::AuthenticatorRef;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::CommandId;
use carosign_core::ids::OperationId;
use carosign_core::ids::WorkerInstanceId;
use carosign_core::operation::context::BootDeadline;
use carosign_core::operation::context::VerificationPurpose;

/// The record is unverified: the fields themselves do not confirm the origin of the worker,
/// recipient or call.
/// The full set is limited by the same limit as the provider wire.
#[derive(Debug, PartialEq, Eq)]
pub struct ProviderAssertionRecord {
    pub operation: OperationId,
    pub worker: WorkerInstanceId,
    pub command: CommandId,
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorRef,
    pub purpose: VerificationPurpose,
    pub recipient: RecipientRef,
    pub challenge: ChallengeRef,
    pub deadline: BootDeadline,
    pub permitted: Vec<EnrollmentRef>,
    /// Amount of full set, including opaque ligaments of the provider.
    pub permitted_digest: [u8; 32],
    pub matched: Option<EnrollmentRef>,
}

impl ProviderAssertionRecord {
    pub(super) fn valid(&self) -> bool {
        !self.permitted.is_empty()
            && self.permitted.len() <= 32
            && self
                .permitted
                .iter()
                .all(|entry| entry.authenticator == self.authenticator.id)
            && !self.permitted.iter().enumerate().any(|(i, entry)| {
                self.permitted[..i]
                    .iter()
                    .any(|previous| previous.id == entry.id)
            })
            && self
                .matched
                .is_none_or(|entry| self.permitted.contains(&entry))
            && self.challenge.is_single_use()
    }
}
