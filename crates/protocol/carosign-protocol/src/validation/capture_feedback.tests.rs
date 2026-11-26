use super::*;
use crate::carosign::common::v1::{AcceptedSamples, CaptureRetry};

#[test]
fn retry_reason_and_prompt_must_match_bound_device() {
    let retry = |reason, prompt| CaptureFeedback {
        kind: Some(Kind::CaptureRetry(CaptureRetry {
            reason: Some(reason as i32),
            prompt,
        })),
    };
    for (reason, modality) in [
        (Reason::PartialCapture, Modality::Fingerprint),
        (Reason::PoorLighting, Modality::Face),
        (Reason::TooQuiet, Modality::Voice),
    ] {
        assert!(capture_feedback_matches_device(
            &retry(reason, None),
            modality,
            false
        ));
    }
    assert!(!capture_feedback_matches_device(
        &retry(Reason::TooQuiet, None),
        Modality::Fingerprint,
        false,
    ));
    let swipe = retry(
        Reason::SwipeTooShort,
        Some(CapturePrompt {
            action: Action::Swipe as i32,
            target: Target::Finger as i32,
        }),
    );
    assert!(!capture_feedback_matches_device(
        &swipe,
        Modality::Fingerprint,
        false
    ));
    assert!(capture_feedback_matches_device(
        &swipe,
        Modality::Fingerprint,
        true
    ));
    assert!(!check_capture_feedback(&retry(
        Reason::PoorQuality,
        Some(CapturePrompt {
            action: Action::Speak as i32,
            target: Target::Finger as i32,
        })
    )));
}

#[test]
fn progress_has_one_valid_unit() {
    let feedback = |kind| CaptureFeedback { kind: Some(kind) };
    assert!(check_capture_feedback(&feedback(Kind::Percent(0))));
    assert!(!check_capture_feedback(&feedback(Kind::Percent(101))));
    for (accepted, required, valid) in [(0, 0, false), (1, 2, true), (3, 2, false)] {
        assert_eq!(
            check_capture_feedback(&feedback(Kind::AcceptedSamples(AcceptedSamples {
                accepted,
                required,
            }))),
            valid,
        );
    }
    assert!(!check_capture_feedback(&CaptureFeedback::default()));
}

#[test]
fn feedback_requires_an_advertised_event_kind() {
    let feedback = CaptureFeedback {
        kind: Some(Kind::Percent(10)),
    };
    let mut device = CapabilityDescriptor {
        modality: Modality::Fingerprint as i32,
        supported_events: vec![CaptureEventKind::CaptureEventPrompt as i32],
        ..Default::default()
    };
    assert!(!capture_feedback_is_declared(&feedback, &device));
    device
        .supported_events
        .push(CaptureEventKind::CaptureEventProgress as i32);
    assert!(capture_feedback_is_declared(&feedback, &device));
}
