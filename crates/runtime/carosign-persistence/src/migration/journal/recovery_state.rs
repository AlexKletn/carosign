use super::{Snapshot, Stage};

/// The read phase does not allow automatic continuation of the migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum RecoveryState {
    /// v3 marker without a journal: migration has not yet been published.
    Unstarted,
    /// Pending/Staged requires a separate administrative decision.
    AdministrativeRecovery(Stage),
    /// A fixed result v4; the staging removal is performed separately.
    Committed(Snapshot),
}
