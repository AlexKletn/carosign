use super::*;
use carosign_protocol::carosign::common::v1::{CaptureFeedback, CapturePrompt, CaptureRetry};

#[test]
fn retry_uses_the_specific_reason_or_fingerprint_prompt() {
    for (reason, action, expected) in [
        (
            Some(CaptureRetryReason::SwipeTooShort),
            None,
            CaptureStatus::SwipeTooShort,
        ),
        (
            Some(CaptureRetryReason::PartialCapture),
            Some(CaptureAction::Center),
            CaptureStatus::FingerNotCentered,
        ),
        (
            None,
            Some(CaptureAction::Remove),
            CaptureStatus::RemoveAndRetry,
        ),
        (None, None, CaptureStatus::Retry),
    ] {
        let event = OperationEvent {
            kind: EventKind::EventRetry as i32,
            feedback: Some(CaptureFeedback {
                kind: Some(Kind::CaptureRetry(CaptureRetry {
                    reason: reason.map(|reason| reason as i32),
                    prompt: action.map(|action| CapturePrompt {
                        action: action as i32,
                        target: CaptureTarget::Finger as i32,
                    }),
                })),
            }),
            ..Default::default()
        };
        assert_eq!(retry_status(&event), Some(expected));
    }
}
