use super::super::*;
use crate::ports::QuiesceAssessment;
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::WorkerInstanceId;

fn instance(byte: u8) -> WorkerInstanceId {
    WorkerInstanceId::from_bytes([byte; 16]).expect("валид")
}

#[test]
fn new_worker_gets_new_epoch() {
    let mut supervisor = Supervisor::new();
    let first = supervisor.start(instance(1), DeviceEpoch::FIRST).unwrap();
    assert_eq!(first.value(), 2);
    supervisor.lost().unwrap();
    let second = supervisor.start(instance(2), first).unwrap();
    assert_eq!(second.value(), 3);
}

#[test]
fn stale_or_exhausted_epoch_never_reuses_an_old_fence() {
    let mut supervisor = Supervisor::new();
    let current = supervisor.start(instance(1), DeviceEpoch::FIRST).unwrap();
    supervisor.lost().unwrap();
    let before = supervisor.current().cloned();
    assert_eq!(
        supervisor.start(instance(2), DeviceEpoch::FIRST),
        Err(SupervisorError::StaleEpoch)
    );
    assert_eq!(supervisor.current(), before.as_ref());
    assert_eq!(
        supervisor.start(instance(2), DeviceEpoch::new(u64::MAX).unwrap()),
        Err(SupervisorError::EpochExhausted)
    );
    assert_eq!(supervisor.current(), before.as_ref());
    assert_eq!(
        supervisor.start(instance(2), current).unwrap().value(),
        current.value() + 1
    );
}

#[test]
fn loss_means_uncertain_hardware() {
    let mut supervisor = Supervisor::new();
    supervisor.start(instance(1), DeviceEpoch::FIRST).unwrap();
    supervisor.mark_ready().unwrap();
    let (_, assessment) = supervisor.lost().unwrap();
    assert_eq!(assessment, QuiesceAssessment::Uncertain);
    assert!(!supervisor.is_ready());
}

#[test]
fn quarantine_requires_a_worker() {
    let mut supervisor = Supervisor::new();
    assert_eq!(supervisor.quarantined(), Err(SupervisorError::NotRunning));
    assert!(supervisor.current().is_none());
}
