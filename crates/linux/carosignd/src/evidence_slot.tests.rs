use super::*;
use carosign_core::ids::BootId;
use carosign_core::ids::{EvidenceId, OperationId};
use carosign_core::operation::context::{BootDeadline, BootInstant};
use carosign_evidence::material::EvidenceMaterialRef;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

fn fixture() -> (EvidenceSlot, BootInstant) {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    let now = BootInstant::new(boot, 10);
    (
        EvidenceSlot::new(
            EvidenceId::from_bytes([2; 16]).unwrap(),
            OperationId::from_bytes([3; 16]).unwrap(),
            BootDeadline::new(boot, 20),
        ),
        now,
    )
}

fn other_reference(id: [u8; 16], operation: [u8; 16], now: BootInstant) -> EvidenceMaterialRef {
    let slot = EvidenceSlot::new(
        EvidenceId::from_bytes(id).unwrap(),
        OperationId::from_bytes(operation).unwrap(),
        BootDeadline::new(now.boot(), 20),
    );
    slot.hold(Zeroizing::new(vec![1]), now).unwrap()
}

#[test]
fn unissued_reference_cannot_confirm_destruction() {
    let (slot, now) = fixture();
    let reference = other_reference([2; 16], [3; 16], now);
    assert_eq!(
        slot.invalidate(reference.clone()),
        Err(SlotError::WrongReference)
    );
    assert_eq!(
        slot.hold(Zeroizing::new(vec![9]), BootInstant::new(now.boot(), 20)),
        Err(SlotError::Expired)
    );
    assert_eq!(
        slot.invalidate(reference.clone()),
        Err(SlotError::WrongReference)
    );
}

#[test]
fn store_binds_unique_refs_limits_capacity_and_waits_for_actual_wipe() {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    let now = BootInstant::new(boot, 10);
    let deadline = BootDeadline::new(boot, 20);
    assert!(EvidenceStore::new(0).is_none());
    assert!(EvidenceStore::new(crate::operations::OPERATIONS_MAX + 1).is_none());
    let store = EvidenceStore::new(2).unwrap();
    let id = EvidenceId::from_bytes([2; 16]).unwrap();
    let operation = OperationId::from_bytes([3; 16]).unwrap();
    let reference = store
        .hold(id, operation, deadline, Zeroizing::new(vec![42]), now)
        .unwrap();
    assert!(matches!(
        store.hold(
            id,
            OperationId::from_bytes([4; 16]).unwrap(),
            deadline,
            Zeroizing::new(vec![1]),
            now
        ),
        Err(SlotError::Unavailable)
    ));
    assert!(matches!(
        store.hold(
            EvidenceId::from_bytes([4; 16]).unwrap(),
            operation,
            deadline,
            Zeroizing::new(vec![1]),
            now
        ),
        Err(SlotError::Unavailable)
    ));
    let foreign = other_reference(reference.id(), [4; 16], now);
    assert_eq!(store.invalidate(foreign), Err(SlotError::WrongReference));
    let lease = store.take(reference.clone(), now).unwrap();
    assert_eq!(lease.bytes(), &[42]);
    assert!(matches!(
        store.take(reference.clone(), now),
        Err(SlotError::Unavailable)
    ));
    assert_eq!(store.invalidate(reference.clone()), Err(SlotError::InUse));
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    drop(lease);
    assert_eq!(store.invalidate(reference.clone()), Ok(()));
    assert!(matches!(
        store.hold(id, operation, deadline, Zeroizing::new(vec![1]), now),
        Err(SlotError::Unavailable)
    ));
    let second = store
        .hold(
            EvidenceId::from_bytes([5; 16]).unwrap(),
            OperationId::from_bytes([6; 16]).unwrap(),
            deadline,
            Zeroizing::new(vec![1]),
            now,
        )
        .unwrap();
    assert!(matches!(
        store.hold(
            EvidenceId::from_bytes([7; 16]).unwrap(),
            OperationId::from_bytes([8; 16]).unwrap(),
            deadline,
            Zeroizing::new(vec![1]),
            now
        ),
        Err(SlotError::Unavailable)
    ));
    store.expire(BootInstant::new(boot, 20));
    assert_eq!(store.invalidate(second), Ok(()));
}

#[test]
fn invalidation_waits_for_checker_and_suppresses_late_result() {
    let (slot, now) = fixture();
    let reference = slot.hold(Zeroizing::new(vec![42; 16]), now).unwrap();
    let foreign = other_reference(reference.id(), [4; 16], now);
    assert_eq!(slot.invalidate(foreign), Err(SlotError::WrongReference));
    let lease = slot.take(reference.clone(), now).unwrap();
    assert_eq!(lease.bytes(), &[42; 16]);
    assert!(matches!(
        slot.take(reference.clone(), now),
        Err(SlotError::Unavailable)
    ));
    assert_eq!(slot.invalidate(reference.clone()), Err(SlotError::InUse));
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    drop(lease);
    assert_eq!(slot.invalidate(reference.clone()), Ok(()));
    assert!(matches!(
        slot.take(reference.clone(), now),
        Err(SlotError::Unavailable)
    ));
}

#[test]
fn expiry_foreign_boot_and_lost_owner_revoke_material() {
    let (slot, now) = fixture();
    let reference = slot.hold(Zeroizing::new(vec![7]), now).unwrap();
    let lease = slot.take(reference.clone(), now).unwrap();
    let foreign = BootInstant::new(BootId::from_bytes([9; 16]).unwrap(), 11);
    assert_eq!(slot.expire(foreign), Err(SlotError::InUse));
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    drop(slot);
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    drop(lease);
}

#[test]
fn expiry_observed_during_check_permanently_revokes_lease() {
    let (slot, now) = fixture();
    let reference = slot.hold(Zeroizing::new(vec![7]), now).unwrap();
    let lease = slot.take(reference.clone(), now).unwrap();
    let expired = BootInstant::new(now.boot(), 20);
    assert_eq!(lease.still_valid(expired), Err(SlotError::Expired));
    assert_eq!(lease.still_valid(now), Err(SlotError::Revoked));
    assert_eq!(slot.invalidate(reference.clone()), Err(SlotError::InUse));
    assert_eq!(lease.finish(now), Err(SlotError::Revoked));
    assert_eq!(slot.invalidate(reference.clone()), Ok(()));
}

#[test]
fn expired_second_take_revokes_in_flight_check() {
    let (slot, now) = fixture();
    let reference = slot.hold(Zeroizing::new(vec![7]), now).unwrap();
    let lease = slot.take(reference.clone(), now).unwrap();
    assert!(matches!(
        slot.take(reference.clone(), BootInstant::new(now.boot(), 20)),
        Err(SlotError::Expired)
    ));
    assert_eq!(lease.finish(now), Err(SlotError::Revoked));
    assert_eq!(slot.invalidate(reference.clone()), Ok(()));
}

#[test]
fn expiry_before_acceptance_cannot_be_undone_by_older_time() {
    let (slot, now) = fixture();
    assert_eq!(
        slot.expire(BootInstant::new(now.boot(), 20)),
        Err(SlotError::Unavailable)
    );
    assert!(matches!(
        slot.hold(Zeroizing::new(vec![7]), now),
        Err(SlotError::Unavailable)
    ));
}

#[test]
fn expiry_of_held_material_blocks_a_later_take_with_stale_time() {
    let (slot, now) = fixture();
    let reference = slot.hold(Zeroizing::new(vec![7]), now).unwrap();
    assert_eq!(slot.expire(BootInstant::new(now.boot(), 20)), Ok(()));
    assert!(matches!(
        slot.take(reference.clone(), now),
        Err(SlotError::Unavailable)
    ));
    assert_eq!(slot.invalidate(reference.clone()), Ok(()));
}

#[test]
fn size_and_deadline_checked_before_reference_is_issued() {
    let (slot, now) = fixture();
    assert!(matches!(
        slot.hold(Zeroizing::new(vec![]), now),
        Err(SlotError::Unavailable)
    ));
    assert!(matches!(
        slot.hold(
            Zeroizing::new(vec![0; carosign_protocol::limits::OPAQUE_TOKEN_MAX + 1]),
            now
        ),
        Err(SlotError::Unavailable)
    ));
    let expired = now.deadline_after(1);
    let later = BootInstant::new(expired.boot(), expired.instant_ns() + 10);
    assert!(matches!(
        slot.hold(Zeroizing::new(vec![1]), later),
        Err(SlotError::Expired)
    ));
    assert!(matches!(
        slot.hold(Zeroizing::new(vec![1]), now),
        Err(SlotError::Unavailable)
    ));
}

// The test examiner is tied to a pre-agreed area.
// This is not a production implementation or a source of hardware warranties.
struct TestVerifier {
    slot: Arc<EvidenceStore>,
    expected: carosign_runtime::ports::EvidenceContext,
    now: Mutex<BootInstant>,
    pause: Option<Arc<std::sync::Barrier>>,
}

impl carosign_runtime::ports::EvidencePort for TestVerifier {
    fn evaluate<'a>(
        &'a self,
        material: EvidenceMaterialRef,
        context: &'a carosign_runtime::ports::EvidenceContext,
        requirements: &'a carosign_evidence::requirements::EvidenceRequirements,
    ) -> carosign_runtime::BoxFuture<
        'a,
        Result<
            carosign_core::assurance::AssuranceAssessment,
            carosign_runtime::ports::EvidenceRefusal,
        >,
    > {
        use carosign_core::assurance::{AssuranceAssessment, AssuranceClass};
        use carosign_runtime::ports::EvidenceRefusal;
        Box::pin(async move {
            let expected = &self.expected;
            if material.operation() != *context.operation.as_bytes()
                || context.operation != expected.operation
                || context.subject != expected.subject
                || context.authenticator != expected.authenticator
                || context.enrollment != expected.enrollment
                || context.worker != expected.worker
                || context.permitted != expected.permitted
                || context.purpose != expected.purpose
                || context.recipient != expected.recipient
                || context.challenge != expected.challenge
                || context.deadline != expected.deadline
                || requirements.purpose != context.purpose
                || !requirements.accepts(AssuranceClass::SoftwareAsserted)
                || !context
                    .permitted
                    .bindings()
                    .iter()
                    .any(|b| b.enrollment == context.enrollment)
            {
                return Err(EvidenceRefusal::Unverified);
            }
            let now = *self.now.lock().unwrap();
            let lease = self
                .slot
                .take(material.clone(), now)
                .map_err(|error| match error {
                    SlotError::Expired => EvidenceRefusal::Expired,
                    _ => EvidenceRefusal::Unverified,
                })?;
            if lease.bytes() != b"test-evidence" {
                return Err(EvidenceRefusal::Unverified);
            }
            if let Some(barrier) = &self.pause {
                barrier.wait();
                barrier.wait();
            }
            lease
                .finish(*self.now.lock().unwrap())
                .map_err(|error| match error {
                    SlotError::Expired => EvidenceRefusal::Expired,
                    _ => EvidenceRefusal::Unverified,
                })?;
            Ok(AssuranceAssessment::record(
                EvidenceId::try_from(material.id().as_slice()).unwrap(),
                AssuranceClass::SoftwareAsserted,
                context.operation,
                context.enrollment.id,
                context.authenticator.id,
            ))
        })
    }

    fn invalidate(
        &self,
        material: EvidenceMaterialRef,
    ) -> carosign_runtime::BoxFuture<'_, Result<(), carosign_runtime::ports::EvidenceRefusal>>
    {
        Box::pin(async move {
            self.slot
                .invalidate(material)
                .map_err(|_| carosign_runtime::ports::EvidenceRefusal::Unavailable)
        })
    }
}

fn verifier_fixture(
    pause: Option<Arc<std::sync::Barrier>>,
) -> (Arc<TestVerifier>, EvidenceMaterialRef) {
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::ids::{AuthenticatorId, EnrollmentId, WorkerInstanceId};
    use carosign_runtime::operation::bindings::{BindingKind, EnrollmentBinding, PermittedSet};
    let now = BootInstant::new(BootId::from_bytes([1; 16]).unwrap(), 10);
    let slot = Arc::new(EvidenceStore::new(1).unwrap());
    let reference = slot
        .hold(
            EvidenceId::from_bytes([2; 16]).unwrap(),
            OperationId::from_bytes([3; 16]).unwrap(),
            BootDeadline::new(now.boot(), 20),
            Zeroizing::new(b"test-evidence".to_vec()),
            now,
        )
        .unwrap();
    let initial = carosign_engine::tests_support::initial();
    let authenticator = carosign_core::device::AuthenticatorRef {
        id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
        device_epoch: carosign_core::device::DeviceEpoch::FIRST,
    };
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([5; 16]).unwrap(),
        authenticator: authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let permitted = PermittedSet::new(vec![EnrollmentBinding {
        enrollment,
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![1],
        format_id: None,
        format_version: 0,
    }])
    .unwrap();
    let context = carosign_runtime::ports::EvidenceContext {
        operation: OperationId::from_bytes([3; 16]).unwrap(),
        subject: initial.common.context.subject,
        authenticator,
        enrollment,
        worker: WorkerInstanceId::from_bytes([6; 16]).unwrap(),
        permitted,
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
        recipient: carosign_evidence::recipient::RecipientRef::new([7; 16]).unwrap(),
        challenge: carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([8; 16])
            .unwrap(),
        deadline: BootDeadline::new(now.boot(), 20),
    };
    (
        Arc::new(TestVerifier {
            slot,
            expected: context,
            now: Mutex::new(now),
            pause,
        }),
        reference,
    )
}

#[test]
fn test_port_accepts_only_exact_context_once_and_never_hardware() {
    use carosign_core::assurance::AssuranceClass;
    use carosign_evidence::requirements::EvidenceRequirements;
    use carosign_runtime::ports::{EvidencePort, EvidenceRefusal};
    let (port, reference) = verifier_fixture(None);
    let requirements = EvidenceRequirements::local_login();
    let mut foreign = port.expected.clone();
    foreign.worker = carosign_core::ids::WorkerInstanceId::from_bytes([9; 16]).unwrap();
    assert!(matches!(
        futures_executor::block_on(port.evaluate(reference.clone(), &foreign, &requirements)),
        Err(EvidenceRefusal::Unverified)
    ));
    let mut foreign_contexts = Vec::new();
    let mut foreign = port.expected.clone();
    foreign.operation = OperationId::from_bytes([9; 16]).unwrap();
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.subject.authority = carosign_core::ids::AuthorityId::from_bytes([9; 16]).unwrap();
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.authenticator.device_epoch = carosign_core::device::DeviceEpoch::new(2).unwrap();
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.enrollment.generation =
        carosign_core::enrollment::EnrollmentGeneration::new(2).unwrap();
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.recipient = carosign_evidence::recipient::RecipientRef::new([9; 16]).unwrap();
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.challenge =
        carosign_evidence::recipient::ChallengeRef::accepted_by_recipient([9; 16]).unwrap();
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.purpose = carosign_core::operation::context::VerificationPurpose::SessionUnlock;
    foreign_contexts.push(foreign);
    let mut foreign = port.expected.clone();
    foreign.deadline = BootDeadline::new(foreign.deadline.boot(), 21);
    foreign_contexts.push(foreign);
    for foreign in &foreign_contexts {
        assert!(matches!(
            futures_executor::block_on(port.evaluate(reference.clone(), foreign, &requirements)),
            Err(EvidenceRefusal::Unverified)
        ));
    }
    let mut hardware = requirements;
    hardware.hardware_required = true;
    assert!(matches!(
        futures_executor::block_on(port.evaluate(reference.clone(), &port.expected, &hardware)),
        Err(EvidenceRefusal::Unverified)
    ));
    let assessment =
        futures_executor::block_on(port.evaluate(reference.clone(), &port.expected, &requirements))
            .unwrap();
    assert_eq!(assessment.class(), AssuranceClass::SoftwareAsserted);
    assert!(matches!(
        futures_executor::block_on(port.evaluate(reference.clone(), &port.expected, &requirements)),
        Err(EvidenceRefusal::Unverified)
    ));
    assert!(futures_executor::block_on(port.invalidate(reference.clone())).is_ok());
}

#[test]
fn test_port_rejects_late_positive_after_concurrent_revocation() {
    use carosign_runtime::ports::{EvidencePort, EvidenceRefusal};
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let (port, reference) = verifier_fixture(Some(barrier.clone()));
    let worker_port = port.clone();
    let worker_reference = reference.clone();
    let worker = std::thread::spawn(move || {
        futures_executor::block_on(worker_port.evaluate(
            worker_reference,
            &worker_port.expected,
            &carosign_evidence::requirements::EvidenceRequirements::local_login(),
        ))
    });
    barrier.wait();
    assert_eq!(
        futures_executor::block_on(port.invalidate(reference.clone())),
        Err(EvidenceRefusal::Unavailable)
    );
    barrier.wait();
    assert!(matches!(
        worker.join().unwrap(),
        Err(EvidenceRefusal::Unverified)
    ));
    assert_eq!(
        futures_executor::block_on(port.invalidate(reference.clone())),
        Ok(())
    );
}
