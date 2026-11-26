use carosign_core::device::AuthenticatorRef;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::CommandId;
use carosign_core::ids::EvidenceId;
use carosign_core::ids::IdSource;
use carosign_core::ids::OperationId;
use carosign_core::ids::WorkerInstanceId;
use carosign_core::operation::context::BootDeadline;
use carosign_core::operation::context::BootInstant;
use carosign_core::operation::context::VerificationPurpose;
use zeroize::Zeroizing;

use super::super::ProviderAssertionRecord;
use super::*;
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::ids::*;

struct TestIds;
impl IdSource for TestIds {
    fn new_operation_id(&self) -> Result<OperationId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_mutation_id(&self) -> Result<MutationId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_command_id(&self) -> Result<CommandId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_worker_instance_id(&self) -> Result<WorkerInstanceId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_lease_id(&self) -> Result<LeaseId, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_evidence_id(&self) -> Result<EvidenceId, IdUnavailable> {
        Ok(EvidenceId::from_bytes([2; 16]).unwrap())
    }
    fn new_idempotency_key(&self) -> Result<IdempotencyKey, IdUnavailable> {
        Err(IdUnavailable)
    }
    fn new_connection_id(&self) -> Result<ConnectionId, IdUnavailable> {
        Err(IdUnavailable)
    }
}

fn scope() -> (BootInstant, BootDeadline, EvidenceId, OperationId) {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    (
        BootInstant::new(boot, 10),
        BootDeadline::new(boot, 20),
        EvidenceId::from_bytes([2; 16]).unwrap(),
        OperationId::from_bytes([3; 16]).unwrap(),
    )
}

#[test]
fn assertion_is_one_shot_and_revocation_waits_for_owner() {
    let (now, deadline, id, operation) = scope();
    let store = EvidenceStore::new(1).unwrap();
    let authenticator = AuthenticatorRef {
        id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
        device_epoch: DeviceEpoch::new(1).unwrap(),
    };
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([5; 16]).unwrap(),
        authenticator: authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let mut record = ProviderAssertionRecord {
        operation,
        worker: WorkerInstanceId::from_bytes([6; 16]).unwrap(),
        command: CommandId::from_bytes([7; 16]).unwrap(),
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([8; 16]).unwrap(),
            subject: SubjectId::from_bytes([9; 16]).unwrap(),
        },
        authenticator,
        purpose: VerificationPurpose::SessionLogin,
        recipient: crate::recipient::RecipientRef::new([10; 16]).unwrap(),
        challenge: crate::recipient::ChallengeRef::accepted_by_recipient([11; 16]).unwrap(),
        deadline,
        permitted: vec![enrollment],
        permitted_digest: [13; 32],
        matched: None,
    };
    record.permitted.push(enrollment);
    assert!(!record.valid());
    record.permitted.pop();
    record.matched = Some(EnrollmentRef {
        id: EnrollmentId::from_bytes([12; 16]).unwrap(),
        ..enrollment
    });
    assert!(!record.valid());
    record.matched = None;
    let reference = store.hold_assertion(&TestIds, record, now).unwrap();
    assert_eq!(reference.id(), *id.as_bytes());
    assert!(store.take(reference.clone(), now).is_err());
    let lease = store.take_assertion(reference.clone(), now).unwrap();
    assert_eq!(lease.record().permitted, vec![enrollment]);
    assert_eq!(lease.record().matched, None);
    assert_eq!(lease.record().challenge.id(), [11; 16]);
    assert!(store.take_assertion(reference.clone(), now).is_err());
    assert_eq!(store.invalidate(reference.clone()), Err(SlotError::InUse));
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    drop(lease);
    assert_eq!(store.invalidate(reference.clone()), Ok(()));
    store.release_after_handoff(&reference).unwrap();
    assert!(store.take_assertion(reference, now).is_err());
}

#[test]
fn stale_ref_and_foreign_owner_never_touch_reused_pair() {
    let (now, deadline, id, operation) = scope();
    let store = EvidenceStore::new(1).unwrap();
    let old = store
        .hold(id, operation, deadline, Zeroizing::new(vec![1]), now)
        .unwrap();
    assert_eq!(store.invalidate(old.clone()), Ok(()));
    // Until handoff, the container remains occupied, although the bytes are cleared.
    assert!(matches!(
        store.hold(id, operation, deadline, Zeroizing::new(vec![2]), now),
        Err(SlotError::Unavailable)
    ));
    store.release_after_handoff(&old).unwrap();
    let current = store
        .hold(id, operation, deadline, Zeroizing::new(vec![2]), now)
        .unwrap();
    assert_ne!(old, current);
    assert_eq!(
        store.invalidate(old.clone()),
        Err(SlotError::WrongReference)
    );
    assert_eq!(
        store.release_after_handoff(&old),
        Err(SlotError::WrongReference)
    );
    assert!(matches!(
        store.take(old, now),
        Err(SlotError::WrongReference)
    ));
    let other = EvidenceStore::new(1).unwrap();
    let foreign = other
        .hold(id, operation, deadline, Zeroizing::new(vec![9]), now)
        .unwrap();
    assert_eq!(store.invalidate(foreign), Err(SlotError::WrongReference));
    assert_eq!(store.take(current, now).unwrap().bytes(), &[2]);
}

#[test]
fn capacity_reuses_after_handoff_and_never_wraps() {
    let (now, deadline, id, operation) = scope();
    let store = EvidenceStore::new(1).unwrap();
    for number in 1u16..=300 {
        let mut bytes = [0; 16];
        bytes[..2].copy_from_slice(&number.to_be_bytes());
        let operation = OperationId::from_bytes(bytes).unwrap();
        let reference = store
            .hold(id, operation, deadline, Zeroizing::new(vec![1]), now)
            .unwrap();
        let lease = store.take(reference.clone(), now).unwrap();
        assert_eq!(store.invalidate(reference.clone()), Err(SlotError::InUse));
        assert_eq!(
            store.release_after_handoff(&reference),
            Err(SlotError::InUse)
        );
        drop(lease);
        assert_eq!(store.invalidate(reference.clone()), Ok(()));
        store.release_after_handoff(&reference).unwrap();
    }
    store.inner.lock().unwrap().generation = u64::MAX;
    assert!(matches!(
        store.hold(id, operation, deadline, Zeroizing::new(vec![1]), now),
        Err(SlotError::GenerationExhausted)
    ));
}

#[test]
fn simultaneous_limit_is_enforced_before_any_reuse() {
    let (now, deadline, _, _) = scope();
    let store = EvidenceStore::new(STORE_MAX).unwrap();
    for number in 1u16..=256 {
        let mut id = [0; 16];
        id[..2].copy_from_slice(&number.to_be_bytes());
        let operation = OperationId::from_bytes(id).unwrap();
        let evidence = EvidenceId::from_bytes(id).unwrap();
        store
            .hold(evidence, operation, deadline, Zeroizing::new(vec![1]), now)
            .unwrap();
    }
    let extra = [9; 16];
    assert!(matches!(
        store.hold(
            EvidenceId::from_bytes(extra).unwrap(),
            OperationId::from_bytes(extra).unwrap(),
            deadline,
            Zeroizing::new(vec![1]),
            now
        ),
        Err(SlotError::Unavailable)
    ));
}

#[test]
fn stale_reference_cannot_match_a_new_store() {
    let (now, deadline, id, operation) = scope();
    let first = EvidenceStore::new(1).unwrap();
    let old = first
        .hold(id, operation, deadline, Zeroizing::new(vec![1]), now)
        .unwrap();
    drop(first);
    let second = EvidenceStore::new(1).unwrap();
    let current = second
        .hold(id, operation, deadline, Zeroizing::new(vec![2]), now)
        .unwrap();
    assert_ne!(old, current);
    assert_eq!(second.invalidate(old), Err(SlotError::WrongReference));
    assert_eq!(second.take(current, now).unwrap().bytes(), &[2]);
}
