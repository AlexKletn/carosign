use super::ReconciliationFailure;
use super::RegistryAction;
use super::VerifiedState;

/// Outcome of independent verification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReconciliationVerdict {
    /// Confirmed **desirable ** condition (registration created/deleted).
    DesiredStateConfirmed(VerifiedState),
    /// **The original state has been confirmed (nothing has changed).
    PreStateConfirmed(VerifiedState),
    /// Nothing's been established.
    Indeterminate(ReconciliationFailure),
}

impl ReconciliationVerdict {
    /// Does the outcome permit the transition to `Committing`.
    ///
    /// ⚠️ `Indeterminate` - No: the transition to fixation is prohibited.
    pub fn allows_commit(self) -> bool {
        !matches!(self, Self::Indeterminate(_))
    }

    /// What to do with the registry when fixing.
    pub fn registry_action(self) -> Option<RegistryAction> {
        match self {
            Self::DesiredStateConfirmed(_) => Some(RegistryAction::ApplyDesiredState),
            Self::PreStateConfirmed(_) => Some(RegistryAction::RecordNoChange),
            Self::Indeterminate(_) => None,
        }
    }
}
