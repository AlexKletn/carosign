use super::*;

#[test]
fn readiness_requires_ordered_handshake_and_fault_is_terminal() {
    assert_eq!(
        next_phase(WorkerPhase::Starting, WorkerEvent::AuthenticatorReady),
        None
    );
    let phase = next_phase(WorkerPhase::Starting, WorkerEvent::Hello).unwrap();
    assert_eq!(next_phase(phase, WorkerEvent::Hello), None);
    let phase = next_phase(phase, WorkerEvent::Initialized).unwrap();
    let phase = next_phase(phase, WorkerEvent::AuthenticatorReady).unwrap();
    assert_eq!(phase, WorkerPhase::Ready);
    assert_eq!(next_phase(phase, WorkerEvent::AuthenticatorReady), None);
    let faulted = next_phase(phase, WorkerEvent::Lost).unwrap();
    assert_eq!(faulted, WorkerPhase::Faulted);
    assert_eq!(next_phase(faulted, WorkerEvent::Hello), None);
    assert_eq!(next_phase(faulted, WorkerEvent::AuthenticatorReady), None);
}
