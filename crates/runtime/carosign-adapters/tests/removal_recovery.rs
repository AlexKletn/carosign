//! Restore removal without repeating the command to the device.

use carosign_adapters::mutation::restore_removal_for_reconciliation;
use carosign_adapters::recovery::{StartupRecoveryError, prepare_startup_recovery_v3};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{CommandId, EnrollmentId, MutationId, WorkerInstanceId};
use carosign_engine::input::PlannedEffect;
use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
use carosign_engine::{EffectSpec, OperationPhase};
use carosign_persistence::model::{MutationPhase, MutationRecord};
use carosign_persistence::mutation_plan::{
    BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition,
};
use carosign_persistence::outbox::{EffectOutboxRecord, EffectPhase};
use carosign_persistence::store::{MetadataStore, Table, tests_support::MemoryStore};

fn put_idempotency(
    tx: &mut dyn carosign_persistence::store::WriteTransaction,
    record: &MutationRecord,
) {
    use carosign_core::ids::IdempotencyKey;
    use carosign_persistence::model::IdempotencyRecord;
    let entry = IdempotencyRecord {
        caller_digest: [21; 32],
        key: IdempotencyKey::from_bytes(*record.mutation.as_bytes()).unwrap(),
        request_digest: [23; 32],
        operation: record.operation,
        mutation: Some(record.mutation),
    };
    let key = carosign_persistence::redb::codec::idempotency_key(&entry.caller_digest, &entry.key);
    tx.put(
        Table::Idempotency,
        &key,
        &carosign_persistence::mutation_records::encode_idempotency(&entry).unwrap(),
    )
    .unwrap();
}

#[test]
fn armed_removal_recovers_exact_plan_only_for_matching_scope_and_epoch() {
    let context = carosign_engine::tests_support::initial().common.context;
    let mutation = MutationId::from_bytes([31; 16]).unwrap();
    let old = EnrollmentRef {
        id: EnrollmentId::from_bytes([32; 16]).unwrap(),
        authenticator: context.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let plan = MutationPlan::build(
        Transition::Remove,
        Some(old),
        None,
        0,
        vec![BindingSnapshot {
            enrollment: old,
            subject: context.subject,
            kind: StoredBindingKind::ProviderOpaque,
            provider_ref_digest: [4; 32],
            format_id: "example".into(),
            format_version: 1,
        }],
        Scope {
            subject: context.subject,
            authenticator: context.authenticator.id,
        },
    )
    .unwrap();
    let command = CommandId::from_bytes([33; 16]).unwrap();
    let record = MutationRecord {
        mutation,
        operation: context.operation_id,
        subject: context.subject,
        authenticator: context.authenticator.id,
        expected_generation: Some(old.generation),
        phase: MutationPhase::DispatchArmed,
        worker: Some(WorkerInstanceId::from_bytes([34; 16]).unwrap()),
        epoch: Some(context.authenticator.device_epoch),
        command: Some(command),
        recovery_ref: None,
        provider_observation_digest: None,
        terminal_outcome: None,
        plan: Some(plan),
    };
    let outbox = EffectOutboxRecord {
        key: EffectKey {
            operation: context.operation_id,
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: Some(command),
        phase: EffectPhase::Armed,
        mutation: Some(mutation),
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    put_idempotency(&mut *tx, &record);
    tx.put(
        Table::Mutations,
        mutation.as_bytes(),
        &carosign_persistence::mutation_records::encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        mutation.as_bytes(),
    )
    .unwrap();
    tx.put(
        Table::EffectOutbox,
        &outbox.storage_key(),
        &carosign_persistence::mutation_records::encode_outbox(&outbox).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    // An empty current base does not prove that there is a change for reconciliation.
    let temp_dir =
        std::env::temp_dir().join(format!("carosign-adapter-v4-{}", std::process::id()));
    std::fs::create_dir(&temp_dir).unwrap();
    let path = temp_dir.join("store.redb");
    let store_v4 = carosign_persistence::redb::backend::RedbStore::create_current(&path).unwrap();
    drop(store_v4);
    let store_v4 =
        carosign_persistence::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    assert!(
        carosign_adapters::mutation::restore_removal_for_reconciliation_v4(
            &store_v4,
            context.clone(),
            mutation
        )
        .is_err()
    );
    drop(store_v4);
    std::fs::remove_dir_all(temp_dir).unwrap();
    let state = restore_removal_for_reconciliation(&store, context.clone(), mutation).unwrap();
    assert_eq!(state.common.phase, OperationPhase::Reconciling);
    assert!(state.mutation().unwrap().plan.is_some());
    assert!(
        !state
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
    );
    assert!(!carosign_engine::recovery::may_replay_mutating(&state));
    use carosign_runtime::recovery::blocked_scope::{
        RecoveryCoordinator, RecoveryError, RecoveryScope,
    };
    let scope = RecoveryScope {
        subject: context.subject,
        authenticator: context.authenticator.id,
    };
    let mut coordinator = RecoveryCoordinator::new();
    let mut actor = carosign_runtime::OperationActor::new(state.clone());
    actor
        .post(carosign_runtime::OperationMsg::OwnerDetached)
        .unwrap();
    let mut failure = coordinator.register(actor).unwrap_err();
    assert_eq!(failure.reason, RecoveryError::NotBlocked);
    assert!(!matches!(
        failure.actor.run_once(),
        carosign_runtime::operation::actor::Step::Idle
    ));
    use carosign_runtime::operation::registry::{OperationRegistry, Owner, TransferError};
    use carosign_runtime::ports::{CallerContext, TransactionId};
    let owner = Owner {
        caller: CallerContext::from_transport(
            [7; 32],
            1000,
            TransactionId::from_bytes([7; 16]).unwrap(),
        ),
    };
    let mut actor = carosign_runtime::OperationActor::new(state.clone());
    actor
        .post(carosign_runtime::OperationMsg::OwnerDetached)
        .unwrap();
    let mut registry = OperationRegistry::new();
    let id = registry.insert(actor, owner.clone()).unwrap();
    assert_eq!(
        registry.transfer_to_recovery(id, &mut coordinator),
        Err(TransferError::Recovery(RecoveryError::NotBlocked))
    );
    let (mut returned, returned_owner) = registry.take(id).unwrap();
    assert_eq!(returned_owner, owner);
    assert!(!matches!(
        returned.run_once(),
        carosign_runtime::operation::actor::Step::Idle
    ));
    registry.put(returned, returned_owner);
    coordinator.block(scope, mutation).unwrap();
    registry.transfer_to_recovery(id, &mut coordinator).unwrap();
    assert!(registry.contains(id));
    assert_eq!(registry.recovery_owner(id), Some(&owner));
    assert!(coordinator.has_actor(scope));
    assert!(registry.take(id).is_err());
    let (scan, mut coordinator) = prepare_startup_recovery_v3(&store).unwrap();
    assert_eq!(scan.needing_reconciliation, vec![mutation]);
    assert!(coordinator.is_blocked(scope));
    for effect in [
        EffectSpec::PersistIntent,
        EffectSpec::PersistDispatchArmed,
        EffectSpec::SendMutatingCommand,
    ] {
        let mut stale = state.clone();
        stale
            .common
            .pending_effects
            .push(PlannedEffect::at(effect, TransitionIndex::new(8), 0));
        assert!(matches!(
            coordinator.register(carosign_runtime::OperationActor::new(stale)),
            Err(
                carosign_runtime::recovery::blocked_scope::RegistrationFailure {
                    reason: RecoveryError::InvalidActor,
                    ..
                }
            )
        ));
        assert!(coordinator.is_blocked(scope));
        assert!(!coordinator.has_actor(scope));
    }
    let mut mismatched = state.clone();
    mismatched.common.context.subject.subject =
        carosign_core::ids::SubjectId::from_bytes([91; 16]).unwrap();
    assert!(matches!(
        coordinator.register(carosign_runtime::OperationActor::new(mismatched)),
        Err(
            carosign_runtime::recovery::blocked_scope::RegistrationFailure {
                reason: RecoveryError::InvalidActor,
                ..
            }
        )
    ));
    coordinator
        .register(carosign_runtime::OperationActor::new(state.clone()))
        .unwrap();
    assert!(coordinator.is_blocked(scope));
    assert!(coordinator.has_actor(scope));
    assert!(matches!(
        coordinator.register(carosign_runtime::OperationActor::new(state)),
        Err(
            carosign_runtime::recovery::blocked_scope::RegistrationFailure {
                reason: RecoveryError::Conflict,
                ..
            }
        )
    ));
    let mut changed_epoch = context.clone();
    changed_epoch.authenticator.device_epoch =
        DeviceEpoch::new(context.authenticator.device_epoch.value() + 1).unwrap();
    assert!(restore_removal_for_reconciliation(&store, changed_epoch, mutation).is_err());
    let mut wrong_subject = context;
    wrong_subject.subject.subject = carosign_core::ids::SubjectId::from_bytes([90; 16]).unwrap();
    assert!(restore_removal_for_reconciliation(&store, wrong_subject, mutation).is_err());

    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        prepare_startup_recovery_v3(&store),
        Err(StartupRecoveryError::Storage(_))
    ));
}

#[test]
fn startup_rejects_more_scopes_than_the_coordinator_can_hold() {
    use carosign_core::ids::{OperationId, SubjectId};
    use carosign_runtime::recovery::blocked_scope::{RECOVERY_SCOPES_MAX, RecoveryError};

    let context = carosign_engine::tests_support::initial().common.context;
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    for number in 1..=(RECOVERY_SCOPES_MAX + 1) as u8 {
        let mut subject = context.subject;
        subject.subject = SubjectId::from_bytes([number; 16]).unwrap();
        let mutation = MutationId::from_bytes([number; 16]).unwrap();
        let old = EnrollmentRef {
            id: EnrollmentId::from_bytes([number; 16]).unwrap(),
            authenticator: context.authenticator.id,
            generation: EnrollmentGeneration::FIRST,
        };
        let plan = MutationPlan::build(
            Transition::Remove,
            Some(old),
            None,
            0,
            vec![BindingSnapshot {
                enrollment: old,
                subject,
                kind: StoredBindingKind::ProviderOpaque,
                provider_ref_digest: [4; 32],
                format_id: "example".into(),
                format_version: 1,
            }],
            Scope {
                subject,
                authenticator: context.authenticator.id,
            },
        )
        .unwrap();
        let record = MutationRecord {
            mutation,
            operation: OperationId::from_bytes([number; 16]).unwrap(),
            subject,
            authenticator: context.authenticator.id,
            expected_generation: Some(old.generation),
            phase: MutationPhase::Prepared,
            worker: None,
            epoch: None,
            command: None,
            recovery_ref: None,
            provider_observation_digest: None,
            terminal_outcome: None,
            plan: Some(plan),
        };
        put_idempotency(&mut *tx, &record);
        tx.put(
            Table::Mutations,
            mutation.as_bytes(),
            &carosign_persistence::mutation_records::encode_mutation(&record).unwrap(),
        )
        .unwrap();
        tx.put(
            Table::MutationScopeLock,
            &record.scope_key(),
            mutation.as_bytes(),
        )
        .unwrap();
    }
    tx.commit_durable().unwrap();
    assert!(matches!(
        prepare_startup_recovery_v3(&store),
        Err(StartupRecoveryError::Coordinator(RecoveryError::Limit))
    ));
}
