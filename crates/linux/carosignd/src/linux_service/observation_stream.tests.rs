use super::*;

#[test]
fn gap_resynchronizes_at_the_snapshot_sequence() {
    let operation = OperationId::from_bytes([7; 16]).unwrap();
    let snapshot = OperationSnapshot {
        operation_id: operation.as_bytes().to_vec(),
        phase: wire::OperationPhase::OpRunning as i32,
        last_event_sequence: 333,
        terminal: None,
    };
    let frame = gap_frame(operation, 12, snapshot.clone());
    assert!(frame.request_id.is_empty());
    assert_eq!(frame.operation_id, operation.as_bytes().as_slice());
    assert_eq!(frame.event_sequence, 333);
    let Some(Payload::OperationEvent(event)) = frame.payload else {
        panic!("expected operation event");
    };
    assert_eq!(event.kind, wire::EventKind::EventGap as i32);
    assert_eq!(event.sequence, frame.event_sequence);
    assert_eq!(event.gap_after_sequence, 12);
    assert_eq!(event.snapshot, Some(snapshot));
    assert!(event.terminal.is_none());
}
