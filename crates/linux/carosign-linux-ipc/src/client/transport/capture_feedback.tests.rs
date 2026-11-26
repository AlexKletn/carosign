use super::*;
use carosign_protocol::carosign::common::v1::{CapturePrompt, CaptureRetry};

#[test]
fn unknown_retry_details_preserve_the_retry_and_sequence() {
    let mut event = OperationEvent {
        kind: EventKind::EventRetry as i32,
        sequence: 17,
        feedback: Some(CaptureFeedback {
            kind: Some(Kind::CaptureRetry(CaptureRetry {
                reason: Some(999),
                prompt: Some(CapturePrompt {
                    action: 999,
                    target: 999,
                }),
            })),
        }),
        ..Default::default()
    };
    normalize(&mut event, true).unwrap();
    assert_eq!(event.sequence, 17);
    assert_eq!(
        event.feedback.unwrap().kind,
        Some(Kind::CaptureRetry(CaptureRetry::default())),
    );
}

#[test]
fn terminal_and_unnegotiated_events_cannot_carry_feedback() {
    for (kind, negotiated) in [
        (EventKind::EventTerminal, true),
        (EventKind::EventProgress, false),
    ] {
        let mut event = OperationEvent {
            kind: kind as i32,
            feedback: Some(CaptureFeedback {
                kind: Some(Kind::Percent(10)),
            }),
            ..Default::default()
        };
        assert!(normalize(&mut event, negotiated).is_err());
    }
}
