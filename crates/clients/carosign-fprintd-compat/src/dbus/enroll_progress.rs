use super::EnrollOutcome;

/// Enrollment updates produced by the backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnrollProgress {
    Pending,
    Capture {
        present: Option<bool>,
        needed: Option<bool>,
    },
    Retry(super::CaptureStatus),
    StagePassed,
    Stages(u32),
    Completed(EnrollOutcome),
}
