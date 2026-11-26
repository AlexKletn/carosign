use carosign_protocol::carosign::{
    client::v1::EventKind,
    common::v1::{CaptureAction, CaptureFeedback, capture_feedback::Kind},
};
use carosign_runtime::operation::events::Progress;

pub(super) fn encode_progress(progress: Progress) -> (EventKind, u32, Option<CaptureFeedback>) {
    use EventKind::*;
    let (kind, percent) = match progress {
        Progress::PlaceFinger => (EventPlaceFinger, 0),
        Progress::MoveFinger => (EventMoveFinger, 0),
        Progress::RemoveFinger => (EventRemoveFinger, 0),
        Progress::Retry => (EventRetry, 0),
        Progress::CaptureState(_) | Progress::Working => (EventWorking, 0),
        Progress::Capturing(value) => (EventProgress, u32::from(value.value())),
        Progress::Feedback(feedback) => {
            let (kind, percent) = match feedback.kind {
                Some(Kind::CaptureRetry(_)) => (EventRetry, 0),
                Some(Kind::Percent(percent)) => (EventProgress, percent),
                Some(Kind::Prompt(prompt)) => match CaptureAction::from_i32(prompt.action) {
                    Some(CaptureAction::Place) => (EventPlaceFinger, 0),
                    Some(CaptureAction::Move) => (EventMoveFinger, 0),
                    Some(CaptureAction::Remove) => (EventRemoveFinger, 0),
                    _ => (EventWorking, 0),
                },
                _ => (EventWorking, 0),
            };
            return (kind, percent, Some(feedback));
        }
    };
    (kind, percent, None)
}
