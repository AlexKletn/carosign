use super::*;
use carosign_protocol::carosign::client::v1::{
    OperationEvent, OperationPhase, TerminalResult, VerificationResult, VerificationTerminal,
};

fn accepted() -> OperationAccepted {
    OperationAccepted {
        operation_id: vec![1; 16],
        first_event_sequence: 1,
        deadline: None,
    }
}

fn frame(sequence: u64, terminal: bool) -> ClientFrame {
    ClientFrame {
        major: 1,
        minor: 0,
        request_id: vec![2; 16],
        operation_id: vec![1; 16],
        event_sequence: sequence,
        payload: Some(Payload::OperationEvent(OperationEvent {
            capture_state: None,
            feedback: None,
            operation_id: vec![1; 16],
            sequence,
            kind: if terminal {
                EventKind::EventTerminal as i32
            } else {
                EventKind::EventWorking as i32
            },
            phase: OperationPhase::OpRunning as i32,
            percent: 0,
            gap_after_sequence: 0,
            snapshot: None,
            terminal: terminal.then_some(TerminalResult {
                kind: Some(Kind::Verification(VerificationTerminal {
                    matched_enrollment: None,
                    result: VerificationResult::VerifyFactorAccepted as i32,
                    error: None,
                })),
            }),
        })),
    }
}

#[test]
fn operation_order_and_terminal_are_bound_without_grant() {
    let mut events = VerificationEvents::new(&accepted()).unwrap();
    let mut wrong = frame(1, false);
    wrong.operation_id[0] = 3;
    assert_eq!(events.accept(&wrong), Err(InvalidEvent));
    assert_eq!(events.accept(&frame(1, false)), Err(InvalidEvent));
    let mut events = VerificationEvents::new(&accepted()).unwrap();
    assert_eq!(events.accept(&frame(2, false)), Err(InvalidEvent));
    let mut events = VerificationEvents::new(&accepted()).unwrap();
    assert_eq!(events.accept(&frame(1, false)), Ok(false));
    assert_eq!(events.accept(&frame(2, true)), Ok(true));
    assert_eq!(events.accept(&frame(3, false)), Err(InvalidEvent));
}

#[test]
fn last_sequence_may_be_terminal_but_not_nonterminal() {
    let mut terminal = VerificationEvents::new(&OperationAccepted {
        first_event_sequence: u64::MAX,
        ..accepted()
    })
    .unwrap();
    assert_eq!(terminal.accept(&frame(u64::MAX, true)), Ok(true));
    assert_eq!(terminal.accept(&frame(u64::MAX, true)), Err(InvalidEvent));

    let mut unfinished = VerificationEvents::new(&OperationAccepted {
        first_event_sequence: u64::MAX,
        ..accepted()
    })
    .unwrap();
    assert_eq!(
        unfinished.accept(&frame(u64::MAX, false)),
        Err(InvalidEvent)
    );
}

#[test]
fn unknown_terminal_result_cannot_be_accepted() {
    let mut event = frame(1, true);
    let Some(Payload::OperationEvent(event)) = &mut event.payload else {
        unreachable!()
    };
    let Some(Kind::Verification(terminal)) = event.terminal.as_mut().and_then(|t| t.kind.as_mut())
    else {
        unreachable!()
    };
    terminal.result = 999;
    let mut tracker = VerificationEvents::new(&accepted()).unwrap();
    // Check the entire external frame after the change in the final outcome.
    let mut outer = frame(1, true);
    outer.payload = Some(Payload::OperationEvent(event.clone()));
    assert_eq!(tracker.accept(&outer), Err(InvalidEvent));
}
