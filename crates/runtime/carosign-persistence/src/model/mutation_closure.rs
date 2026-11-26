use carosign_core::operation::mutation::MutationFailure;

/// The Closing Change: What to Do with the Sphere
///
/// ⚠️ Separated from `MutationFailure`: there's a job failure, here's the state of the
/// magazine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MutationClosure {
    /// The change is closed and confirmed.
    Completed,
    /// There was no change.
    NoChange,
    /// You need a check; the lock remains.
    NeedsReconciliation,
}

impl MutationClosure {
    /// Is it liberating the area?
    ///
    /// ⚠️ `NeedsReconciliation` **No releases (document 23).
    pub fn releases_scope(self) -> bool {
        matches!(self, Self::Completed | Self::NoChange)
    }

    /// Refusal if the closure fails.
    pub fn failure(self) -> Option<MutationFailure> {
        match self {
            Self::Completed | Self::NoChange => None,
            Self::NeedsReconciliation => Some(MutationFailure::SecureStoreRejected),
        }
    }
}

#[cfg(test)]
#[path = "mutation_closure.tests.rs"]
mod tests;
