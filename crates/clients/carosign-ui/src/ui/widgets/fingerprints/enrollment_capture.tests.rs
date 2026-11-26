use carosign_protocol::carosign::common::v1::{AcceptedSamples, CaptureFeedback, CaptureRetry};

use super::*;

#[test]
fn prompts_and_retries_keep_the_last_accepted_progress() {
    let mut capture = EnrollmentCapture::new(3);
    let feedback = |kind| OperationEvent {
        feedback: Some(CaptureFeedback { kind: Some(kind) }),
        ..OperationEvent::default()
    };
    capture.apply(&feedback(Kind::AcceptedSamples(AcceptedSamples {
        accepted: 1,
        required: 4,
    })));
    assert_eq!(capture.percent, 25.0);
    capture.apply(&feedback(Kind::CaptureRetry(CaptureRetry {
        reason: Some(CaptureRetryReason::PartialCapture as i32),
        prompt: None,
    })));
    assert_eq!(capture.percent, 25.0);
    assert!(capture.retry);
    assert_eq!(capture.message, "ui-enroll-event-partial");
    capture.apply(&feedback(Kind::Prompt(CapturePrompt {
        action: CaptureAction::Remove as i32,
        target: CaptureTarget::Finger as i32,
    })));
    assert_eq!(capture.percent, 25.0);
    assert!(!capture.retry);
    assert_eq!(capture.message, "ui-enroll-event-remove");
}
