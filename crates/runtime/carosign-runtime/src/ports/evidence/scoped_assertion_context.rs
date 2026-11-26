use carosign_core::identity::SubjectRef;

/// The area of independent evaluation of a provider’s approval without a token.
/// The structure itself does not confirm the origin of either the caller or the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedAssertionContext {
    /// Subject and device of authorized operation.
    pub subject: SubjectRef,
    pub authenticator: carosign_core::device::AuthenticatorRef,
    /// Appointment, recipient, one-time call and deadline.
    pub purpose: carosign_core::operation::context::VerificationPurpose,
    pub recipient: carosign_evidence::recipient::RecipientRef,
    pub challenge: carosign_evidence::recipient::ChallengeRef,
    pub deadline: carosign_core::operation::context::BootDeadline,
}
