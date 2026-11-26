/// Selection of a session already prepared by the composition prior to atomic tolerance.
#[derive(Clone, Copy)]
pub(crate) struct VerificationFlowTarget {
    pub worker: carosign_core::ids::WorkerInstanceId,
    pub command: carosign_core::ids::CommandId,
    pub authenticator: carosign_core::device::AuthenticatorRef,
    pub provider_subject: carosign_core::identity::SubjectRef,
    pub uid: u32,
    pub purpose: carosign_core::operation::context::VerificationPurpose,
    pub challenge: carosign_evidence::recipient::ChallengeRef,
    pub recipient: carosign_evidence::recipient::RecipientRef,
}
