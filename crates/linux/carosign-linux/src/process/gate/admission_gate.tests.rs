use std::time::Duration;

use super::super::{AdmissionError, AdmissionGate, Gate, child_guard::ChildGuard};

#[test]
fn admission_and_revoke_have_one_serialized_winner() {
    use std::sync::{Arc, Barrier};
    for _ in 0..32 {
        let (sender, receiver) = Gate::new().unwrap().into_parts();
        let owner = Arc::new(AdmissionGate::new(7, sender).unwrap());
        assert_eq!(owner.admit(8), Err(AdmissionError::StaleGeneration));
        let barrier = Arc::new(Barrier::new(3));
        let a = Arc::clone(&owner);
        let b = Arc::clone(&barrier);
        let admit = std::thread::spawn(move || {
            b.wait();
            a.admit(7)
        });
        let a = Arc::clone(&owner);
        let b = Arc::clone(&barrier);
        let revoke = std::thread::spawn(move || {
            b.wait();
            a.revoke()
        });
        barrier.wait();
        let admitted = admit.join().unwrap().is_ok();
        let revoked = revoke.join().unwrap().is_ok();
        assert_ne!(admitted, revoked);
        assert_eq!(receiver.wait(Duration::from_secs(1)).is_ok(), admitted);
        assert_eq!(owner.admit(7), Err(AdmissionError::Closed));
    }
}

#[test]
fn accepted_revoke_denies_release_to_separate_process() {
    use std::process::{Command, Stdio};
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    let owner = AdmissionGate::new(3, sender).unwrap();
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process::gate::tests::gate_child", "--ignored"])
            .stdin(Stdio::from(receiver.into_owned()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    assert_eq!(owner.revoke(), Ok(()));
    assert_eq!(owner.admit(3), Err(AdmissionError::Closed));
    assert!(child.0.wait().unwrap().success());
}
