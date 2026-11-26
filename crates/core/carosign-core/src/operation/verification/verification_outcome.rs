use super::VerificationFailure;
use crate::ids::EvidenceId;

/// How did the check-up for the platform end?
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The provider claims to have matched; the link to the proof has not yet been verified.
    Matched(EvidenceId),
    NotMatched,
    /// The finger was not/failed to be removed.
    NoUsableSample,
    Cancelled,
    /// Seems like a denial.
    Failed(VerificationFailure),
}

impl VerificationOutcome {
    /// Does the outcome want proof checks?
    pub fn needs_evidence_check(&self) -> bool {
        matches!(self, Self::Matched(_))
    }

    /// Is the outcome a success for the user?
    ///
    /// ⚠️ Always. `false` even `Matched`The decision is made by the verifying evidence and
    /// policy, not by the provider.
    pub fn is_user_success(&self) -> bool {
        false
    }
}
