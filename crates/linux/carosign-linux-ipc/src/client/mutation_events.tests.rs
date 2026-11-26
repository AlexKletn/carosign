use super::*;
#[test]
fn mismatched_operation_and_missing_result_are_rejected() {
    let accepted = OperationAccepted {
        operation_id: vec![1; 16],
        first_event_sequence: 1,
        deadline: None,
    };
    let mut tracker = MutationEvents::new(&accepted).unwrap();
    let mut frame = ClientFrame {
        operation_id: vec![2; 16],
        event_sequence: 1,
        payload: Some(Payload::OperationEvent(
            carosign_protocol::carosign::client::v1::OperationEvent {
                feedback: None,
                operation_id: vec![1; 16],
                sequence: 1,
                kind: EventKind::EventTerminal as i32,
                ..Default::default()
            },
        )),
        ..Default::default()
    };
    assert!(tracker.accept(&frame).is_err());
    frame.operation_id = vec![1; 16];
    assert!(
        MutationEvents::new(&accepted)
            .unwrap()
            .accept(&frame)
            .is_err()
    );
}
