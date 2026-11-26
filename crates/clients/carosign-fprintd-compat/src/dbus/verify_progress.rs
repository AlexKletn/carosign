use super::VerifyOutcome;

/// Verification updates from the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyProgress {
    /// The result arrives asynchronously.
    Pending,
    Capture {
        present: Option<bool>,
        needed: Option<bool>,
    },
    Selected(String),
    Retry(super::CaptureStatus),
    Completed(VerifyOutcome),
}
