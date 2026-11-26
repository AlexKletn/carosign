use carosign_core::identity::SubjectRef;
use carosign_core::ids::OperationId;

/// The full scope of the assessment; the structure itself does not confirm its origin.
#[derive(Clone, Debug)]
pub struct EvidenceContext {
    /// The operation and its subject.
    pub operation: OperationId,
    pub subject: SubjectRef,
    /// Device, era, registration and worker.
    pub authenticator: carosign_core::device::AuthenticatorRef,
    pub enrollment: carosign_core::enrollment::EnrollmentRef,
    pub worker: carosign_core::ids::WorkerInstanceId,
    /// Permitted registrations assigned to this operation.
    pub permitted: crate::operation::bindings::PermittedSet,
    /// Appointment, protected recipient, call and deadline.
    pub purpose: carosign_core::operation::context::VerificationPurpose,
    pub recipient: carosign_evidence::recipient::RecipientRef,
    pub challenge: carosign_evidence::recipient::ChallengeRef,
    pub deadline: carosign_core::operation::context::BootDeadline,
}
