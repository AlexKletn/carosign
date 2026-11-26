use std::time::Duration;

use super::super::Gate;
use super::{MAX_POLICY_ADMISSIONS, PolicyAdmissionError, PolicyAdmissions};

#[test]
fn policy_revoke_closes_every_pending_gate() {
    let owner = PolicyAdmissions::new(7).unwrap();
    let (first, first_receiver) = Gate::new().unwrap().into_parts();
    let (second, second_receiver) = Gate::new().unwrap().into_parts();
    let first_id = owner.register(7, first).unwrap();
    owner.register(7, second).unwrap();
    assert_eq!(
        owner.admit(8, first_id),
        Err(PolicyAdmissionError::StaleGeneration)
    );
    owner.revoke().unwrap();
    assert_eq!(owner.admit(7, first_id), Err(PolicyAdmissionError::Closed));
    assert!(first_receiver.wait(Duration::from_secs(1)).is_err());
    assert!(second_receiver.wait(Duration::from_secs(1)).is_err());
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    assert_eq!(owner.register(7, sender), Err(PolicyAdmissionError::Closed));
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
}

#[test]
fn policy_admission_is_serialized_with_revoke() {
    use std::sync::{Arc, Barrier};
    for _ in 0..32 {
        let owner = Arc::new(PolicyAdmissions::new(7).unwrap());
        let (sender, receiver) = Gate::new().unwrap().into_parts();
        let id = owner.register(7, sender).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let a = Arc::clone(&owner);
        let b = Arc::clone(&barrier);
        let admit = std::thread::spawn(move || {
            b.wait();
            a.admit(7, id)
        });
        let a = Arc::clone(&owner);
        let b = Arc::clone(&barrier);
        let revoke = std::thread::spawn(move || {
            b.wait();
            a.revoke()
        });
        barrier.wait();
        let admitted = admit.join().unwrap().is_ok();
        assert_eq!(revoke.join().unwrap(), Ok(()));
        assert_eq!(receiver.wait(Duration::from_secs(1)).is_ok(), admitted);
        assert_eq!(owner.admit(7, id), Err(PolicyAdmissionError::Closed));
    }
}

#[test]
fn policy_pending_is_bounded_and_ids_are_single_use() {
    let owner = PolicyAdmissions::new(7).unwrap();
    let mut receivers = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..MAX_POLICY_ADMISSIONS {
        let (sender, receiver) = Gate::new().unwrap().into_parts();
        ids.push(owner.register(7, sender).unwrap());
        receivers.push(receiver);
    }
    let (sender, receiver) = Gate::new().unwrap().into_parts();
    assert_eq!(owner.register(7, sender), Err(PolicyAdmissionError::Limit));
    assert!(receiver.wait(Duration::from_secs(1)).is_err());
    owner.admit(7, ids[0]).unwrap();
    assert_eq!(owner.admit(7, ids[0]), Err(PolicyAdmissionError::Unknown));
    receivers.remove(0).wait(Duration::from_secs(1)).unwrap();
    owner.revoke().unwrap();
    for receiver in receivers {
        assert!(receiver.wait(Duration::from_secs(1)).is_err());
    }
}
