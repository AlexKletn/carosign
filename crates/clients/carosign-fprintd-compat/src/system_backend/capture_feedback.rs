use carosign_protocol::carosign::{
    client::v1::{EventKind, OperationEvent},
    common::v1::{CaptureAction, CaptureRetryReason, CaptureTarget, capture_feedback::Kind},
};

use crate::dbus::CaptureStatus;

pub(super) fn retry_status(event: &OperationEvent) -> Option<CaptureStatus> {
    if let Some(Kind::CaptureRetry(retry)) = event.feedback.and_then(|feedback| feedback.kind) {
        if retry.reason == Some(CaptureRetryReason::SwipeTooShort as i32) {
            return Some(CaptureStatus::SwipeTooShort);
        }
        if retry.reason == Some(CaptureRetryReason::TooFast as i32) {
            return Some(CaptureStatus::TooFast);
        }
        return Some(match retry.prompt {
            Some(prompt) if prompt.target == CaptureTarget::Finger as i32 => {
                match CaptureAction::from_i32(prompt.action) {
                    Some(CaptureAction::Center) => CaptureStatus::FingerNotCentered,
                    Some(CaptureAction::Remove) => CaptureStatus::RemoveAndRetry,
                    _ => CaptureStatus::Retry,
                }
            }
            _ => CaptureStatus::Retry,
        });
    }
    (event.kind == EventKind::EventRetry as i32).then_some(CaptureStatus::Retry)
}

#[path = "capture_feedback.tests.rs"]
#[cfg(test)]
mod tests;

pub(super) fn finger_state(
    event: &OperationEvent,
) -> Option<carosign_protocol::carosign::common::v1::CaptureState> {
    event
        .capture_state
        .filter(|state| state.target() == CaptureTarget::Finger)
}
