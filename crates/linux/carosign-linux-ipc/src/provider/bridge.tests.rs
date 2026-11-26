use super::*;
use carosign_protocol::carosign::provider::v1::provider_frame::Payload;

#[test]
fn malformed_operation_event_is_not_silently_ignored() {
    let frame = provider::ProviderFrame {
        operation_id: vec![1; 16],
        payload: Some(Payload::ProviderProgress(provider::ProviderProgress {
            capture_state: None,
            feedback: None,
            operation_sequence: 1,
            kind: provider::ProgressKind::ProgressCapturing as i32,
            percent: 101,
        })),
        ..Default::default()
    };
    assert!(observe(&frame).is_err());
}

#[test]
fn non_operation_frame_is_not_an_observation() {
    let frame = provider::ProviderFrame {
        payload: Some(Payload::WorkerHello(provider::WorkerHello::default())),
        ..Default::default()
    };
    assert_eq!(observe(&frame), Ok(None));
}
