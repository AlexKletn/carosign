use super::super::*;
use super::*;
use crate::enrollment_authorization::{AuthorizedEnrollment, EnrollmentAuthorizationScope};
use crate::enrollment_authorization::{
    ChallengeObservation, EnrollmentRecipientId, ValidatedChallenge,
};
use crate::ports::{CommandPlan, EnrollmentAuthorization, MutationArmRequest};
use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, BootId, CommandId, GrantId, LeaseId, MutationId, OperationId,
    SubjectId, WorkerInstanceId,
};
use carosign_core::operation::context::BootInstant;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::time::Duration;

fn scope_fixture() -> (
    EnrollmentAuthorizationScope,
    ValidatedGrant,
    AuthorizedEnrollment,
) {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([2; 16]).unwrap(),
        subject: SubjectId::from_bytes([3; 16]).unwrap(),
    };
    let authenticator = AuthenticatorRef {
        id: AuthenticatorId::from_bytes([4; 16]).unwrap(),
        device_epoch: DeviceEpoch::FIRST,
    };
    let scope = EnrollmentAuthorizationScope {
        operation: OperationId::from_bytes([5; 16]).unwrap(),
        mutation: MutationId::from_bytes([6; 16]).unwrap(),
        challenge_command: CommandId::from_bytes([7; 16]).unwrap(),
        command: CommandId::from_bytes([8; 16]).unwrap(),
        subject,
        uid: 0,
        authenticator,
        recipient: EnrollmentRecipientId::from_bytes([9; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([10; 16]).unwrap(),
        lease: LeaseId::from_bytes([11; 16]).unwrap(),
        boot,
    };
    let grant = ValidatedGrant::issue_for_port(
        GrantId::from_bytes([12; 16]).unwrap(),
        AuthorityId::from_bytes([13; 16]).unwrap(),
        GrantScope::Enroll {
            subject,
            authenticator: authenticator.id,
            replace: None,
        },
        boot,
        2_000_000_000,
    );
    let observation = ChallengeObservation::new(vec![1], [14; 16], 500, 500).unwrap();
    let challenge = ValidatedChallenge::issued_for_port(
        scope,
        &observation,
        &grant,
        BootInstant::new(boot, 1_000_000_000),
        BootInstant::new(boot, 1_500_000_000),
    )
    .unwrap();
    let authorization = AuthorizedEnrollment::issued_for_port(&challenge, [15; 16]).unwrap();
    (scope, grant, authorization)
}

fn fixture() -> SerializedEnrollmentAdmission {
    fixture_with(|| true)
}

fn fixture_with(
    revoke_sink: impl Fn() -> bool + Send + Sync + 'static,
) -> SerializedEnrollmentAdmission {
    let (scope, _, authorization) = scope_fixture();
    SerializedEnrollmentAdmission::new(scope, &authorization, revoke_sink).unwrap()
}

#[test]
fn actual_durable_ticket_is_bound_to_scope_recipient_and_expiry() {
    use crate::operation::registry::ManualClock;
    use crate::ports::MutationChange;
    use carosign_core::enrollment::EnrollmentLabel;
    use carosign_core::ids::{EnrollmentId, IdempotencyKey};
    use carosign_core::operation::plan::PlanTransition;
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    use carosign_persistence::journal::{ArmMutation, PrepareMutation};
    use carosign_persistence::model::IdempotencyRecord;
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
    use carosign_persistence::redb::backend::RedbStore;

    let (scope, grant, authorization) = scope_fixture();
    let domain_scope = Scope {
        subject: scope.subject,
        authenticator: scope.authenticator.id,
    };
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([16; 16]).unwrap()),
        1,
        Vec::new(),
        domain_scope,
    )
    .unwrap()
    .with_requested_label(domain_scope, "finger")
    .unwrap();
    let core_plan = plan.to_core(domain_scope).unwrap();
    let PlanTransition::Create { planned_new } = core_plan.transition() else {
        panic!("create")
    };
    let input = PrepareMutation {
        mutation: scope.mutation,
        operation: scope.operation,
        subject: scope.subject,
        authenticator: scope.authenticator.id,
        expected_generation: None,
        enroll: true,
        plan,
    };
    let idem = IdempotencyRecord {
        caller_digest: [17; 32],
        key: IdempotencyKey::from_bytes([18; 16]).unwrap(),
        request_digest: carosign_persistence::mutation_plan::canonical_request_digest(
            domain_scope,
            Transition::EnrollNew,
            None,
            Some("finger"),
        )
        .unwrap(),
        operation: scope.operation,
        mutation: Some(scope.mutation),
    };
    let path = std::env::temp_dir().join(format!(
        "carosign-admission-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&path).unwrap();
    let store = RedbStore::create_current(&path.join("store.redb")).unwrap();
    let fresh =
        carosign_persistence::mutation_store::prepare_fresh_arm_v4(&store, &input, &idem).unwrap();
    let arm = ArmMutation {
        mutation: scope.mutation,
        worker: scope.worker,
        epoch: scope.authenticator.device_epoch,
        command: scope.command,
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0);
    let ticket = fresh.arm(&arm, effect).unwrap();
    let mut scope_key = [0; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope_key[..32].copy_from_slice(&scope.subject.to_canonical());
    scope_key[32..].copy_from_slice(scope.authenticator.id.as_bytes());
    let request = MutationArmRequest {
        operation: scope.operation,
        uid: scope.uid,
        effect,
        arm,
        scope: scope_key,
        lease: scope.lease,
        label: Some(EnrollmentLabel::new("finger").unwrap()),
        plan: CommandPlan::Mutate {
            mutation: scope.mutation,
            command: scope.command,
            authorization: EnrollmentAuthorization::Reference {
                reference: *authorization.reference(),
                challenge_digest: *authorization.digest(),
            },
            change: MutationChange::Create { planned_new },
        },
        logical_plan: core_plan,
    };
    assert!(request.matches_ticket(&ticket));
    let clock = ManualClock::new(scope.boot);
    clock.advance(1_000_000_000);
    let gate = SerializedEnrollmentAdmission::new(scope, &authorization, || true).unwrap();
    let mut foreign = request.clone();
    foreign.uid = 1000;
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &grant,
        &clock,
        &store,
        &foreign,
        &ticket,
        || true
    ));
    foreign = request.clone();
    foreign.arm.epoch = DeviceEpoch::new(2).unwrap();
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &grant,
        &clock,
        &store,
        &foreign,
        &ticket,
        || true
    ));
    let mut foreign_scope = scope;
    foreign_scope.recipient = EnrollmentRecipientId::from_bytes([20; 16]).unwrap();
    assert!(!gate.validate_request(
        &foreign_scope,
        &authorization,
        &grant,
        &clock,
        &store,
        &request,
        &ticket,
        || true
    ));
    let mut foreign = request.clone();
    let CommandPlan::Mutate {
        authorization:
            EnrollmentAuthorization::Reference {
                challenge_digest, ..
            },
        ..
    } = &mut foreign.plan
    else {
        panic!("authorization")
    };
    challenge_digest[0] ^= 1;
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &grant,
        &clock,
        &store,
        &foreign,
        &ticket,
        || true
    ));
    let other_grant = ValidatedGrant::issue_for_port(
        GrantId::from_bytes([21; 16]).unwrap(),
        grant.issuer(),
        grant.scope().clone(),
        scope.boot,
        2_000_000_000,
    );
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &other_grant,
        &clock,
        &store,
        &request,
        &ticket,
        || true
    ));
    let replace_grant = ValidatedGrant::issue_for_port(
        grant.id(),
        grant.issuer(),
        GrantScope::Enroll {
            subject: scope.subject,
            authenticator: scope.authenticator.id,
            replace: Some(planned_new),
        },
        scope.boot,
        grant.deadline_ns(),
    );
    assert!(
        authorization
            .check(&scope, &replace_grant, clock.now())
            .is_ok()
    );
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &replace_grant,
        &clock,
        &store,
        &request,
        &ticket,
        || true
    ));
    let mut removal = request.plan.clone();
    let binding = crate::operation::bindings::EnrollmentBinding {
        enrollment: planned_new,
        kind: crate::operation::bindings::BindingKind::ProviderOpaque,
        provider_ref: vec![1],
        format_id: None,
        format_version: 0,
    };
    let CommandPlan::Mutate { change, .. } = &mut removal else {
        panic!("mutation")
    };
    *change = MutationChange::Removal {
        enrollment: planned_new,
        binding: binding.clone(),
    };
    assert!(!authorized_enrollment_plan(&grant, &removal));
    let mut replacement = request.plan.clone();
    let CommandPlan::Mutate { change, .. } = &mut replacement else {
        panic!("mutation")
    };
    *change = MutationChange::Replace {
        planned_new,
        old: binding,
    };
    assert!(authorized_enrollment_plan(&replace_grant, &replacement));
    assert!(!authorized_enrollment_plan(&grant, &replacement));
    let other_replace = ValidatedGrant::issue_for_port(
        grant.id(),
        grant.issuer(),
        GrantScope::Enroll {
            subject: scope.subject,
            authenticator: scope.authenticator.id,
            replace: Some(carosign_core::enrollment::EnrollmentRef {
                id: EnrollmentId::from_bytes([23; 16]).unwrap(),
                ..planned_new
            }),
        },
        scope.boot,
        grant.deadline_ns(),
    );
    assert!(!authorized_enrollment_plan(&other_replace, &replacement));
    // Same ticket with another valid ticket planned_new It does not describe an unchangeable
    // plan.
    let foreign_target = carosign_core::enrollment::EnrollmentRef {
        id: EnrollmentId::from_bytes([22; 16]).unwrap(),
        ..planned_new
    };
    let mut foreign = request.clone();
    foreign.logical_plan = carosign_core::operation::plan::MutationPlan::new(
        scope.subject,
        scope.authenticator.id,
        PlanTransition::Create {
            planned_new: foreign_target,
        },
        Vec::new(),
        [0; 32],
    )
    .and_then(|plan| plan.with_label(Some(EnrollmentLabel::new("finger").unwrap())))
    .unwrap();
    let CommandPlan::Mutate { change, .. } = &mut foreign.plan else {
        panic!("mutation")
    };
    *change = MutationChange::Create {
        planned_new: foreign_target,
    };
    assert!(foreign.matches_ticket(&ticket));
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &grant,
        &clock,
        &store,
        &foreign,
        &ticket,
        || true
    ));
    // Expiry during a trusted inspection does not allow dispatch.
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &grant,
        &clock,
        &store,
        &request,
        &ticket,
        || {
            clock.advance(500_000_000);
            true
        }
    ));
    let valid_clock = ManualClock::new(scope.boot);
    valid_clock.advance(1_000_000_000);
    let mut retained_for_phase_test = None;
    assert_eq!(
        gate.consume(
            &scope,
            &authorization,
            &grant,
            &valid_clock,
            &store,
            &request,
            ticket,
            || true,
            |ticket| {
                retained_for_phase_test = Some(ticket);
                ConsumptionOutcome::NotDispatched
            }
        ),
        Ok(ConsumptionOutcome::NotDispatched)
    );
    // Even a ticket mistakenly held by the recipient does not allow sending after the
    // transfer of the journal to reconciliation.
    assert!(!gate.validate_request(
        &scope,
        &authorization,
        &grant,
        &valid_clock,
        &store,
        &request,
        retained_for_phase_test.as_ref().unwrap(),
        || {
            let mut tx = store.begin_write().unwrap();
            carosign_persistence::mutation_store::stage_needs_reconciliation_v4(
                &mut *tx,
                scope.mutation,
                scope.operation,
            )
            .unwrap();
            tx.commit_durable().unwrap();
            true
        }
    ));
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn enrollment_grant_does_not_authorize_real_remove_ticket() {
    use crate::operation::bindings::{BindingKind, EnrollmentBinding};
    use crate::operation::registry::ManualClock;
    use crate::ports::MutationChange;
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
    use carosign_core::ids::{EnrollmentId, IdempotencyKey};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    use carosign_persistence::journal::{ArmMutation, PrepareMutation};
    use carosign_persistence::model::IdempotencyRecord;
    use carosign_persistence::mutation_plan::{
        BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition,
    };
    use carosign_persistence::redb::backend::RedbStore;
    use carosign_persistence::store::Table;

    let (scope, grant, authorization) = scope_fixture();
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([24; 16]).unwrap(),
        authenticator: scope.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject: scope.subject,
        enrollment,
        display_label: None,
        label: "finger".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1]).unwrap(),
        format_id: "format".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let path = std::env::temp_dir().join(format!(
        "carosign-admission-remove-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&path).unwrap();
    let store = RedbStore::create_current(&path.join("store.redb")).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        enrollment.id.as_bytes(),
        &carosign_persistence::mutation_store::encode_enrollment_v4(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::EnrollmentIndex,
        &carosign_persistence::redb::codec::enrollment_index_key(&record),
        enrollment.id.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let domain_scope = Scope {
        subject: scope.subject,
        authenticator: scope.authenticator.id,
    };
    let plan = MutationPlan::build(
        Transition::Remove,
        Some(enrollment),
        None,
        0,
        vec![BindingSnapshot {
            enrollment,
            subject: scope.subject,
            kind: record.binding_kind,
            provider_ref_digest: carosign_persistence::mutation_plan::sha256(
                record.provider_ref.as_bytes(),
            ),
            format_id: record.format_id.clone(),
            format_version: record.format_version,
        }],
        domain_scope,
    )
    .unwrap();
    let core_plan = plan.to_core(domain_scope).unwrap();
    let input = PrepareMutation {
        mutation: scope.mutation,
        operation: scope.operation,
        subject: scope.subject,
        authenticator: scope.authenticator.id,
        expected_generation: Some(enrollment.generation),
        enroll: false,
        plan,
    };
    let idem = IdempotencyRecord {
        caller_digest: [17; 32],
        key: IdempotencyKey::from_bytes([18; 16]).unwrap(),
        request_digest: carosign_persistence::mutation_plan::canonical_request_digest(
            domain_scope,
            Transition::Remove,
            Some(enrollment),
            None,
        )
        .unwrap(),
        operation: scope.operation,
        mutation: Some(scope.mutation),
    };
    let fresh =
        carosign_persistence::mutation_store::prepare_fresh_arm_v4(&store, &input, &idem).unwrap();
    let arm = ArmMutation {
        mutation: scope.mutation,
        worker: scope.worker,
        epoch: scope.authenticator.device_epoch,
        command: scope.command,
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0);
    let ticket = fresh.arm(&arm, effect).unwrap();
    let mut scope_key = [0; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope_key[..32].copy_from_slice(&scope.subject.to_canonical());
    scope_key[32..].copy_from_slice(scope.authenticator.id.as_bytes());
    let request = MutationArmRequest {
        operation: scope.operation,
        uid: scope.uid,
        effect,
        arm,
        scope: scope_key,
        lease: scope.lease,
        label: None,
        plan: CommandPlan::Mutate {
            mutation: scope.mutation,
            command: scope.command,
            authorization: EnrollmentAuthorization::Reference {
                reference: *authorization.reference(),
                challenge_digest: *authorization.digest(),
            },
            change: MutationChange::Removal {
                enrollment,
                binding: EnrollmentBinding {
                    enrollment,
                    kind: BindingKind::ProviderOpaque,
                    provider_ref: vec![1],
                    format_id: Some("format".into()),
                    format_version: 1,
                },
            },
        },
        logical_plan: core_plan,
    };
    assert!(request.matches_ticket(&ticket));
    carosign_persistence::mutation_store::validate_dispatch_ticket_v4(
        &store,
        &ticket,
        &request.logical_plan,
    )
    .unwrap();
    let clock = ManualClock::new(scope.boot);
    clock.advance(1_000_000_000);
    let gate = SerializedEnrollmentAdmission::new(scope, &authorization, || true).unwrap();
    assert_eq!(
        gate.consume(
            &scope,
            &authorization,
            &grant,
            &clock,
            &store,
            &request,
            ticket,
            || true,
            |_| panic!("removal must not reach enrollment sink")
        ),
        Err(AdmissionError::Invalid)
    );
    drop(store);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn revoke_before_consumption_prevents_dispatch() {
    let gate = fixture();
    gate.revoke().unwrap();
    let mut called = false;
    let outcome = gate.consume_inner(
        (),
        |_| true,
        |_| {
            called = true;
            ConsumptionOutcome::Completed
        },
    );
    assert_eq!(outcome, Err(AdmissionError::Revoked));
    assert!(!called);
}

#[test]
fn dropped_unused_gate_revokes_its_bound_recipient() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let gate = fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    });
    drop(gate);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn concurrent_revoke_during_physical_call_is_uncertain_without_blocking() {
    let revoked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = revoked.clone();
    let gate = Arc::new(fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    }));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (trying_tx, trying_rx) = mpsc::channel();
    let (revoked_tx, revoked_rx) = mpsc::channel();
    let owner = gate.clone();
    let consume = std::thread::spawn(move || {
        owner.consume_inner(
            (),
            |_| true,
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                ConsumptionOutcome::Completed
            },
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let revoker = gate.clone();
    let revoke = std::thread::spawn(move || {
        trying_tx.send(()).unwrap();
        let result = revoker.revoke();
        revoked_tx.send(()).unwrap();
        result
    });
    trying_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(revoked_rx.recv_timeout(Duration::from_secs(5)).is_ok());
    assert_eq!(revoke.join().unwrap(), Err(AdmissionError::Unavailable));
    release_tx.send(()).unwrap();
    assert_eq!(consume.join().unwrap(), Err(AdmissionError::Unavailable));
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert_eq!(gate.revoke(), Ok(()));
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::Revoked)
    );
}

#[test]
fn revoke_after_flush_before_return_never_becomes_success() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let gate = Arc::new(fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    }));
    let (flushed_tx, flushed_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let owner = gate.clone();
    let operation = std::thread::spawn(move || {
        owner.consume_with_completion_hook(
            (),
            |_| true,
            |_| ConsumptionOutcome::Completed,
            || {
                flushed_tx.send(()).unwrap();
                resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            },
        )
    });
    flushed_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(gate.revoke(), Ok(()));
    resume_tx.send(()).unwrap();
    assert_eq!(operation.join().unwrap(), Err(AdmissionError::Unavailable));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn pending_revoke_during_live_check_stops_before_dispatch_and_is_delivered() {
    let revoked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = revoked.clone();
    let gate = Arc::new(fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    }));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner = gate.clone();
    let operation = std::thread::spawn(move || {
        owner.consume_inner(
            (),
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                true
            },
            |_| panic!("dispatch after cancellation"),
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(gate.revoke(), Err(AdmissionError::Unavailable));
    release_tx.send(()).unwrap();
    assert_eq!(operation.join().unwrap(), Err(AdmissionError::Revoked));
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert_eq!(gate.revoke(), Ok(()));
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
}

#[test]
fn pending_revoke_is_delivered_even_if_sink_panics() {
    let revoked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = revoked.clone();
    let gate = Arc::new(fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    }));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner = gate.clone();
    let operation = std::thread::spawn(move || {
        owner.consume_inner(
            (),
            |_| true,
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                panic!("sink failure")
            },
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(gate.revoke(), Err(AdmissionError::Unavailable));
    release_tx.send(()).unwrap();
    assert!(operation.join().is_err());
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::Revoked)
    );
}

#[test]
fn pending_revoke_is_delivered_when_live_check_panics() {
    let revoked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = revoked.clone();
    let gate = Arc::new(fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    }));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner = gate.clone();
    let operation = std::thread::spawn(move || {
        owner.consume_inner(
            (),
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                panic!("live check failure")
            },
            |_| panic!("must not dispatch"),
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(gate.revoke(), Err(AdmissionError::Unavailable));
    release_tx.send(()).unwrap();
    assert!(operation.join().is_err());
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::Revoked)
    );
}

#[test]
fn pending_revoke_failure_keeps_capability_closed_until_confirmed() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let gate = Arc::new(fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst) != 0
    }));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let owner = gate.clone();
    let operation = std::thread::spawn(move || {
        owner.consume_inner(
            (),
            |_| true,
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                ConsumptionOutcome::Completed
            },
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(gate.revoke(), Err(AdmissionError::Unavailable));
    release_tx.send(()).unwrap();
    assert_eq!(operation.join().unwrap(), Err(AdmissionError::Unavailable));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::AlreadyUsed)
    );
    assert_eq!(gate.revoke(), Ok(()));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn unknown_or_panic_cannot_authorize_a_second_call() {
    let gate = fixture();
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Unknown),
        Ok(ConsumptionOutcome::Unknown)
    );
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::AlreadyUsed)
    );
    let revoked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = revoked.clone();
    let gate = fixture_with(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    });
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        gate.consume_inner((), |_| true, |_| panic!("sink"))
    }));
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::Revoked)
    );
    assert_eq!(gate.revoke(), Ok(()));
    assert_eq!(revoked.load(Ordering::SeqCst), 1);
}

#[test]
fn refusal_before_dispatch_is_retryable_but_failed_revoke_is_not() {
    let gate = fixture_with(|| false);
    assert_eq!(
        gate.consume_inner((), |_| false, |_| panic!("must not dispatch")),
        Err(AdmissionError::Invalid)
    );
    assert_eq!(gate.revoke(), Err(AdmissionError::Unavailable));
    assert_eq!(
        gate.consume_inner((), |_| true, |_| ConsumptionOutcome::Completed),
        Err(AdmissionError::AlreadyUsed)
    );
}
