use super::CapturePercent;
use carosign_protocol::carosign::common::v1::CaptureFeedback;

/// Semantic prompt of the provider for the observer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Progress {
    Feedback(CaptureFeedback),
    CaptureState(carosign_protocol::carosign::common::v1::CaptureState),
    /// Put your finger on it.
    PlaceFinger,
    /// Move your finger.
    MoveFinger,
    /// Put your finger away.
    RemoveFinger,
    /// Repeat the action.
    Retry,
    /// We're shooting.
    Capturing(CapturePercent),
    /// The provider is doing the job.
    Working,
}
