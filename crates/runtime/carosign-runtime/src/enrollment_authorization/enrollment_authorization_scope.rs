use carosign_core::device::AuthenticatorRef;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{BootId, CommandId, LeaseId, MutationId, OperationId, WorkerInstanceId};

use super::EnrollmentRecipientId;

/// Reference to one attempt and one resource issued.
/// The fields alone do not prove the authority or existence of extradition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnrollmentAuthorizationScope {
    pub operation: OperationId,
    pub mutation: MutationId,
    pub challenge_command: CommandId,
    pub command: CommandId,
    pub subject: SubjectRef,
    pub uid: u32,
    pub authenticator: AuthenticatorRef,
    /// The recipient is determined by a trusted lineup, not a worker message.
    pub recipient: EnrollmentRecipientId,
    pub worker: WorkerInstanceId,
    pub lease: LeaseId,
    pub boot: BootId,
}
