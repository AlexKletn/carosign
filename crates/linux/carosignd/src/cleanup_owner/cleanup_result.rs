use super::{CleanupFailure, CleanupProgress};

/// The result persists even when the owner of the future is interrupted.
pub enum CleanupResult {
    /// The cleaning machine's answer.
    Completed(Result<CleanupProgress, CleanupFailure>),
    /// Fenced cleaning and delivery of pending operations.
    Scheduled(Result<crate::operations::ScheduledCleanupProgress, CleanupFailure>),
    /// Waiting is interrupted; the state of issuance requires restoration.
    Interrupted,
}
