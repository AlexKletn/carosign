use crate::recipient::ChallengeRef;
use crate::recipient::RecipientRef;
use carosign_core::operation::context::BootDeadline;
use carosign_core::operation::context::VerificationPurpose;
use carosign_engine::VerifiedState;
use core::fmt;

/// The context of the request.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceContext {
    pub operation: [u8; 16],
    /// The unsub (opaque).
    pub subject: [u8; 32],
    /// Form of the checked state: registration and generation.
    pub enrollment: VerifiedState,
    /// Target.
    pub purpose: VerificationPurpose,
    pub recipient: RecipientRef,
    pub challenge: ChallengeRef,
    /// Term.
    pub deadline: BootDeadline,
}

impl fmt::Debug for EvidenceContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvidenceContext")
            .field("purpose", &self.purpose)
            .finish_non_exhaustive()
    }
}
