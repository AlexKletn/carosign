use super::*;

fn session() -> ClientSession {
    use carosign_linux_ipc::credentials::PeerCredentials;
    let credentials = PeerCredentials::from_kernel(1234, 1000, 1000).expect("креды");
    ClientSession::new(credentials, [7u8; 16])
}

#[test]
fn nonterminal_runner_results_never_become_terminal_events() {
    use crate::run::RunOutcome;
    use carosign_core::ids::OperationId;
    use carosign_engine::{OperationPhase, TerminalOutcome};
    let operation = OperationId::from_bytes([1; 16]).unwrap();
    for outcome in [
        RunOutcome::Waiting {
            phase: OperationPhase::Running,
        },
        RunOutcome::StepLimit {
            phase: OperationPhase::Running,
        },
        RunOutcome::Refused {
            phase: OperationPhase::Running,
            reason: "нет подтверждения".into(),
        },
    ] {
        assert!(terminal_if_complete(operation, &outcome, 1).is_none());
    }
    let frame = terminal_if_complete(
        operation,
        &RunOutcome::Terminal(TerminalOutcome::NoSample),
        2,
    )
    .unwrap();
    let frame = carosign_protocol::carosign::client::v1::ClientFrame::decode(frame.as_slice()).unwrap();
    assert_eq!(frame.event_sequence, 2);
    assert!(
        matches!(frame.payload, Some(carosign_protocol::carosign::client::v1::client_frame::Payload::OperationEvent(event)) if event.terminal.is_some())
    );
}

#[test]
fn caller_comes_from_the_session_not_the_frame() {
    let (_request, caller) = handle(
        &session(),
        &carosign_linux_ipc::client::session::hello_frame([1u8; 16], 3, 3),
    )
    .expect("кадр");
    // ⚠️ UID is from the connection creeds.
    assert_eq!(caller.uid(), 1000);
    assert_eq!(caller.transaction().as_bytes(), &[7u8; 16]);
}

#[test]
fn zero_connection_never_becomes_a_fixed_transaction() {
    use carosign_linux_ipc::credentials::PeerCredentials;
    let credentials = PeerCredentials::from_kernel(1234, 1000, 1000).unwrap();
    let session = ClientSession::new(credentials, [0; 16]);
    assert!(matches!(
        handle(
            &session,
            &carosign_linux_ipc::client::session::hello_frame([1; 16], 3, 3)
        ),
        Err(CycleError::Malformed)
    ));
}

#[test]
fn hello_is_not_an_operation() {
    let (request, _caller) = handle(
        &session(),
        &carosign_linux_ipc::client::session::hello_frame([1u8; 16], 3, 3),
    )
    .expect("кадр");
    assert_eq!(authenticator_of(&request), None);
}
