use super::CleanupProgress;

/// The result of the fenced cleaning and issuing from the scheduler's queue.
#[must_use]
pub struct ScheduledCleanupProgress {
    pub(crate) progress: CleanupProgress,
}
