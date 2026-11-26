use super::PreflightFailure;

/// Failure after the start of fixation does not prove that staging has persisted.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CleanupFailure {
    Invalid,
    CommitUnknown,
}

impl From<PreflightFailure> for CleanupFailure {
    fn from(_: PreflightFailure) -> Self {
        Self::Invalid
    }
}
