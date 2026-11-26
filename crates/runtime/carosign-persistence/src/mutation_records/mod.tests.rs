use super::carosign::storage::v1 as wire;
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, EnrollmentId, IdempotencyKey, MutationId, OperationId,
    SubjectId, WorkerInstanceId,
};
use prost::Message;

use crate::idempotency::{Reservation, ReservationError};
use crate::journal::{ArmMutation, PrepareMutation};
use crate::model::{
    IdempotencyRecord, MutationPhase, MutationRecord, MutationTerminalOutcome, StorageError,
};
use crate::mutation_plan::{
    BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition, sha256,
};
use crate::outbox::{EffectOutboxRecord, EffectPhase};
use crate::store::Table;

use super::arm_attempt_owner::{ArmAttemptFailure, ArmAttemptOwner};
use super::durable_arm::arm_durable_v3;
use super::fresh_prepared_arm::prepare_fresh_arm;
use super::reconciliation::persist_needs_reconciliation;
use super::*;

fn put_sample_idempotency(tx: &mut dyn crate::store::WriteTransaction, record: &MutationRecord) {
    let entry = IdempotencyRecord {
        caller_digest: [21; 32],
        key: IdempotencyKey::from_bytes([22; 16]).unwrap(),
        request_digest: [23; 32],
        operation: record.operation,
        mutation: Some(record.mutation),
    };
    let key = crate::redb::codec::idempotency_key(&entry.caller_digest, &entry.key);
    tx.put(
        Table::Idempotency,
        &key,
        &encode_idempotency(&entry).unwrap(),
    )
    .unwrap();
}

fn sample() -> MutationRecord {
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let target = EnrollmentRef {
        id: EnrollmentId::from_bytes([4; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let binding = BindingSnapshot {
        enrollment: target,
        subject,
        kind: StoredBindingKind::ProviderOpaque,
        provider_ref_digest: [5; 32],
        format_id: "example".into(),
        format_version: 1,
    };
    let plan = MutationPlan::build(
        Transition::Remove,
        Some(target),
        None,
        0,
        vec![binding],
        Scope {
            subject,
            authenticator,
        },
    )
    .unwrap();
    MutationRecord {
        mutation: MutationId::from_bytes([6; 16]).unwrap(),
        operation: OperationId::from_bytes([7; 16]).unwrap(),
        subject,
        authenticator,
        expected_generation: Some(target.generation),
        phase: MutationPhase::Prepared,
        worker: None,
        epoch: None,
        command: None,
        recovery_ref: None,
        provider_observation_digest: None,
        terminal_outcome: None,
        plan: Some(plan),
    }
}

fn change(bytes: &[u8], apply: impl FnOnce(&mut wire::MutationRecord)) -> Vec<u8> {
    let payload = crate::enrollment_codec::unenvelope_with(bytes, sha256).unwrap();
    let mut stored = wire::StoredValue::decode(payload.as_slice()).unwrap();
    let Some(wire::stored_value::Value::Mutation(record)) = &mut stored.value else {
        unreachable!()
    };
    apply(record);
    crate::enrollment_codec::envelope_with(&stored.encode_to_vec(), sha256).unwrap()
}

#[test]
fn migration_prepares_mutation_without_losing_durable_plan() {
    use crate::installation_records::{MigratableRecord, prepare_v3_record};
    let record = sample();
    let source = encode_mutation(&record).unwrap();
    let key = record.mutation.as_bytes();
    let output = prepare_v3_record(MigratableRecord::Mutation, key, &source).unwrap();
    let unpack = |bytes: &[u8]| {
        let payload = crate::enrollment_codec::unenvelope_with(bytes, sha256).unwrap();
        wire::StoredValue::decode(payload.as_slice()).unwrap()
    };
    let old = unpack(&source);
    let new = unpack(&output);
    assert_eq!(new.schema_version, 4);
    assert_eq!(new.value, old.value);
    assert!(prepare_v3_record(MigratableRecord::Mutation, &[8; 16], &source).is_err());
}

#[test]
fn individual_recovery_reads_reject_conflicting_idempotency() {
    use crate::recovery::{read_completed_mutation_v3, read_mutation_snapshot_v3};
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    let store = MemoryStore::default();
    let mut mutation = sample();
    let entry = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: mutation.operation,
        mutation: None,
    };
    let key = crate::redb::codec::idempotency_key(&entry.caller_digest, &entry.key);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        mutation.mutation.as_bytes(),
        &encode_mutation(&mutation).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &mutation.scope_key(),
        mutation.mutation.as_bytes(),
    )
    .unwrap();
    tx.put(
        Table::Idempotency,
        &key,
        &encode_idempotency(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        read_mutation_snapshot_v3(&store, mutation.mutation, mutation.operation),
        Err(StorageError::Corrupt { .. })
    ));
    mutation.phase = MutationPhase::Completed;
    mutation.terminal_outcome = Some(MutationTerminalOutcome::NoChange);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        mutation.mutation.as_bytes(),
        &encode_mutation(&mutation).unwrap(),
    )
    .unwrap();
    tx.delete(Table::MutationScopeLock, &mutation.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        read_completed_mutation_v3(&store, mutation.mutation, mutation.operation),
        Err(StorageError::Corrupt { .. })
    ));
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::Idempotency, &key).unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        read_completed_mutation_v3(&store, mutation.mutation, mutation.operation),
        Ok(MutationTerminalOutcome::NoChange)
    );
}

#[test]
fn startup_v3_checks_idempotency_against_mutation_in_same_snapshot() {
    use crate::recovery::StartupScan;
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    let store = MemoryStore::default();
    let mutation = sample();
    let mut entry = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: mutation.operation,
        mutation: Some(mutation.mutation),
    };
    let key = crate::redb::codec::idempotency_key(&entry.caller_digest, &entry.key);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Idempotency,
        &key,
        &encode_idempotency(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        StartupScan::scan_v3(&store),
        Err(StorageError::Corrupt { .. })
    ));
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        mutation.mutation.as_bytes(),
        &encode_mutation(&mutation).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &mutation.scope_key(),
        mutation.mutation.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_ok());
    entry.operation = OperationId::from_bytes([99; 16]).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Idempotency,
        &key,
        &encode_idempotency(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        StartupScan::scan_v3(&store),
        Err(StorageError::Corrupt { .. })
    ));
    entry.operation = mutation.operation;
    entry.mutation = None;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Idempotency,
        &key,
        &encode_idempotency(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        StartupScan::scan_v3(&store),
        Err(StorageError::Corrupt { .. })
    ));
}

#[test]
fn startup_v3_rejects_corrupt_idempotency_record_without_resetting_it() {
    use crate::recovery::StartupScan;
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    let store = MemoryStore::default();
    let record = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: None,
    };
    let key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    let good = encode_idempotency(&record).unwrap();
    let mut payload = crate::enrollment_codec::unenvelope_with(&good, sha256).unwrap();
    payload.extend([0x72, 0x00]);
    let corrupt = crate::enrollment_codec::envelope_with(&payload, sha256).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Idempotency, &key, &corrupt).unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        StartupScan::scan_v3(&store),
        Err(StorageError::Corrupt { .. })
    ));
    let tx = store.begin_read().unwrap();
    assert_eq!(tx.get(Table::Idempotency, &key).unwrap(), Some(corrupt));
}

#[test]
fn prepared_and_armed_round_trip_with_the_authoritative_schema() {
    use carosign_protocol::carosign::storage::v1 as official;
    let mut original = sample();
    for armed in [false, true] {
        if armed {
            original.phase = MutationPhase::DispatchArmed;
            original.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
            original.epoch = Some(DeviceEpoch::FIRST);
            original.command = Some(CommandId::from_bytes([9; 16]).unwrap());
        }
        let encoded = encode_mutation(&original).unwrap();
        assert_eq!(decode_mutation(&encoded), Ok(original.clone()));
        let payload = crate::enrollment_codec::unenvelope_with(&encoded, sha256).unwrap();
        let value = official::StoredValue::decode(payload.as_slice()).unwrap();
        assert_eq!(value.schema_version, 3);
        let Some(official::stored_value::Value::Mutation(record)) = value.value else {
            panic!("нет записи изменения")
        };
        assert_eq!(record.kind, 2);
        assert_eq!(record.phase, if armed { 2 } else { 1 });
        assert_eq!(record.target_enrollment, record.plan.unwrap().old_target);
    }
}

#[test]
fn mismatched_target_kind_fence_or_generation_is_rejected() {
    let prepared = encode_mutation(&sample()).unwrap();
    let bad = [
        change(&prepared, |record| record.kind = 1),
        change(&prepared, |record| record.target_enrollment = None),
        change(&prepared, |record| record.expected_generation = 2),
        change(&prepared, |record| record.terminal_outcome = 1),
        change(&prepared, |record| record.phase = 99),
        change(&prepared, |record| {
            record.plan.as_mut().unwrap().affected_scope_digest[0] ^= 1
        }),
        change(&prepared, |record| record.phase = 2),
    ];
    for bytes in bad {
        assert!(matches!(
            decode_mutation(&bytes),
            Err(StorageError::Corrupt { .. })
        ));
    }
}

#[test]
fn unknown_schema_legacy_checksum_and_incomplete_terminal_are_not_reinterpreted() {
    let record = sample();
    let legacy = crate::redb::record::encode_mutation(&record).unwrap();
    assert!(decode_mutation(&legacy).is_err());
    let bytes = encode_mutation(&record).unwrap();
    let payload = crate::enrollment_codec::unenvelope_with(&bytes, sha256).unwrap();
    let mut value = wire::StoredValue::decode(payload.as_slice()).unwrap();
    value.schema_version = 2;
    let old = crate::enrollment_codec::envelope_with(&value.encode_to_vec(), sha256).unwrap();
    assert_eq!(
        decode_mutation(&old),
        Err(StorageError::UnsupportedSchema { found: 2 })
    );
    let mut unsupported = record;
    unsupported.phase = MutationPhase::Completed;
    assert_eq!(
        encode_mutation(&unsupported),
        Err(StorageError::Corrupt {
            what: "неверная запись изменения v3"
        })
    );
}

#[test]
fn all_v3_phases_preserve_observation_and_terminal_outcomes() {
    let mut record = sample();
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    record.epoch = Some(DeviceEpoch::FIRST);
    record.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    for phase in [
        MutationPhase::DispatchArmed,
        MutationPhase::ProviderObserved,
        MutationPhase::RegistryCommitted,
        MutationPhase::NeedsReconciliation,
        MutationPhase::NoChange,
    ] {
        record.phase = phase;
        record.provider_observation_digest = if phase == MutationPhase::DispatchArmed {
            None
        } else {
            Some([42; 32])
        };
        record.terminal_outcome = None;
        let bytes = encode_mutation(&record).unwrap();
        assert_eq!(decode_mutation(&bytes), Ok(record.clone()));
    }
    record.phase = MutationPhase::Completed;
    for outcome in [
        MutationTerminalOutcome::Committed,
        MutationTerminalOutcome::NoChange,
    ] {
        record.terminal_outcome = Some(outcome);
        let bytes = encode_mutation(&record).unwrap();
        assert_eq!(decode_mutation(&bytes), Ok(record.clone()));
        assert_eq!(
            crate::redb::record::encode_mutation(&record),
            Err(StorageError::Unavailable)
        );
    }
}

#[test]
fn invalid_outcome_observation_or_fence_never_round_trips() {
    let mut record = sample();
    let prepared = encode_mutation(&record).unwrap();
    for bytes in [
        change(&prepared, |value| {
            value.provider_observation_digest = vec![1; 32]
        }),
        change(&prepared, |value| value.terminal_outcome = 2),
        change(&prepared, |value| value.terminal_outcome = 99),
    ] {
        assert!(matches!(
            decode_mutation(&bytes),
            Err(StorageError::Corrupt { .. })
        ));
    }
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    record.epoch = Some(DeviceEpoch::FIRST);
    record.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    record.phase = MutationPhase::ProviderObserved;
    assert!(matches!(
        encode_mutation(&record),
        Err(StorageError::Corrupt { .. })
    ));
    record.provider_observation_digest = Some([42; 32]);
    let observed = encode_mutation(&record).unwrap();
    assert!(
        decode_mutation(&change(&observed, |value| value
            .provider_observation_digest =
            vec![1; 31]))
        .is_err()
    );
    let missing_outcome = change(&observed, |value| value.phase = 5);
    assert!(decode_mutation(&missing_outcome).is_err());
    let nonterminal_outcome = change(&observed, |value| value.terminal_outcome = 1);
    assert!(decode_mutation(&nonterminal_outcome).is_err());
    let missing_worker = change(&observed, |value| value.worker_instance_id.clear());
    assert!(decode_mutation(&missing_worker).is_err());
}

#[test]
fn no_change_before_dispatch_has_no_fabricated_command() {
    use crate::recovery::StartupScan;
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    let store = MemoryStore::default();
    let mut record = sample();
    record.phase = MutationPhase::NoChange;
    let encoded = encode_mutation(&record).unwrap();
    assert_eq!(decode_mutation(&encoded), Ok(record.clone()));
    let mut tx = store.begin_write().unwrap();
    put_sample_idempotency(&mut *tx, &record);
    tx.put(Table::Mutations, record.mutation.as_bytes(), &encoded)
        .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        StartupScan::scan_v3(&store).unwrap().needs_decision,
        vec![record.mutation]
    );
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, record.mutation, record.operation)
            .is_ok()
    );
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    assert!(encode_mutation(&record).is_err());
    record.worker = None;
    record.provider_observation_digest = Some([1; 32]);
    assert!(encode_mutation(&record).is_err());
    record.provider_observation_digest = None;
    record.phase = MutationPhase::Completed;
    record.terminal_outcome = Some(MutationTerminalOutcome::NoChange);
    assert_eq!(
        decode_mutation(&encode_mutation(&record).unwrap()),
        Ok(record.clone())
    );
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_completed_mutation_v3(&store, record.mutation, record.operation)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        crate::recovery::read_completed_mutation_v3(&store, record.mutation, record.operation),
        Ok(MutationTerminalOutcome::NoChange)
    );
    assert!(
        crate::recovery::read_completed_mutation_v3(
            &store,
            record.mutation,
            OperationId::from_bytes([99; 16]).unwrap()
        )
        .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        MutationId::from_bytes([98; 16]).unwrap().as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_completed_mutation_v3(&store, record.mutation, record.operation)
            .is_err()
    );
    let mut other = record.clone();
    other.mutation = MutationId::from_bytes([98; 16]).unwrap();
    other.operation = OperationId::from_bytes([97; 16]).unwrap();
    other.phase = MutationPhase::Prepared;
    other.terminal_outcome = None;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        other.mutation.as_bytes(),
        &encode_mutation(&other).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        crate::recovery::read_completed_mutation_v3(&store, record.mutation, record.operation),
        Ok(MutationTerminalOutcome::NoChange)
    );
    // Another log with the same transaction number should not be accessed when searching
    // directly for MutationIdEven if the outcome is correct in itself.
    let mut duplicate = other.clone();
    duplicate.operation = record.operation;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        duplicate.mutation.as_bytes(),
        &encode_mutation(&duplicate).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_completed_mutation_v3(&store, record.mutation, record.operation,)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        other.mutation.as_bytes(),
        &encode_mutation(&other).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let bogus = EffectOutboxRecord {
        key: carosign_engine::transition::EffectKey {
            operation: record.operation,
            transition_index: carosign_engine::transition::TransitionIndex::new(1),
            position: carosign_engine::transition::EffectPosition::new(0),
        },
        command: Some(CommandId::from_bytes([50; 16]).unwrap()),
        phase: EffectPhase::Armed,
        mutation: Some(record.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EffectOutbox,
        &bogus.storage_key(),
        &encode_outbox(&bogus).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_completed_mutation_v3(&store, record.mutation, record.operation,)
            .is_err()
    );
    record.terminal_outcome = Some(MutationTerminalOutcome::Committed);
    assert!(encode_mutation(&record).is_err());
}

#[test]
fn pre_dispatch_no_change_keeps_lock_and_requires_durable_commit() {
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    let store = MemoryStore::default();
    let mut record = sample();
    record.expected_generation = None;
    record.plan = Some(
        MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            vec![],
            Scope {
                subject: record.subject,
                authenticator: record.authenticator,
            },
        )
        .unwrap(),
    );
    let mut tx = store.begin_write().unwrap();
    put_sample_idempotency(&mut *tx, &record);
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    let entries = tx.scan_prefix(Table::Idempotency, &[]).unwrap();
    tx.delete(Table::Idempotency, &entries[0].0).unwrap();
    assert!(stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation).is_err());
    assert_eq!(
        decode_mutation(
            &tx.get(Table::Mutations, record.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::Prepared
    );
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    assert!(stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation).is_err());
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    let mut other = record.clone();
    other.mutation = MutationId::from_bytes([70; 16]).unwrap();
    other.operation = OperationId::from_bytes([71; 16]).unwrap();
    tx.put(
        Table::Mutations,
        other.mutation.as_bytes(),
        &encode_mutation(&other).unwrap(),
    )
    .unwrap();
    assert!(stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation).is_err());
    assert!(enrollment_projection(&*tx, record.mutation, record.operation).is_err());
    let arm = ArmMutation {
        mutation: record.mutation,
        worker: WorkerInstanceId::from_bytes([72; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([73; 16]).unwrap(),
        recovery_ref: None,
    };
    assert!(
        stage_arm(
            &mut *tx,
            &arm,
            carosign_engine::PlannedEffect::at(
                carosign_engine::EffectSpec::SendMutatingCommand,
                carosign_engine::TransitionIndex::new(1),
                0,
            ),
        )
        .is_err()
    );
    assert_eq!(
        decode_mutation(
            &tx.get(Table::Mutations, record.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::Prepared
    );
    assert!(tx.scan_prefix(Table::EffectOutbox, &[]).unwrap().is_empty());
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    let conflicting = EffectOutboxRecord {
        key: carosign_engine::transition::EffectKey {
            operation: record.operation,
            transition_index: carosign_engine::TransitionIndex::new(1),
            position: carosign_engine::transition::EffectPosition::new(0),
        },
        command: Some(CommandId::from_bytes([33; 16]).unwrap()),
        phase: EffectPhase::Armed,
        mutation: Some(record.mutation),
    };
    tx.put(
        Table::EffectOutbox,
        &conflicting.storage_key(),
        &encode_outbox(&conflicting).unwrap(),
    )
    .unwrap();
    assert!(stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation).is_err());
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    assert!(
        stage_pre_dispatch_no_change(
            &mut *tx,
            record.mutation,
            OperationId::from_bytes([99; 16]).unwrap()
        )
        .is_err()
    );
    stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation).unwrap();
    *store.fail_next_commit.lock().unwrap() = true;
    assert_eq!(tx.commit_durable(), Err(StorageError::Unavailable));
    let read = store.begin_read().unwrap();
    assert_eq!(
        decode_mutation(
            &read
                .get(Table::Mutations, record.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::Prepared
    );
    drop(read);
    let mut tx = store.begin_write().unwrap();
    stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation).unwrap();
    tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    assert_eq!(
        decode_mutation(
            &read
                .get(Table::Mutations, record.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::NoChange
    );
    assert_eq!(
        read.get(Table::MutationScopeLock, &record.scope_key())
            .unwrap()
            .as_deref(),
        Some(record.mutation.as_bytes().as_slice())
    );
    assert!(
        read.scan_prefix(Table::EffectOutbox, &[])
            .unwrap()
            .is_empty()
    );
    drop(read);
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_pre_dispatch_no_change(&mut *tx, record.mutation, record.operation),
        Err(StorageError::GenerationConflict)
    );
    let arm = ArmMutation {
        mutation: record.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = carosign_engine::PlannedEffect::at(
        carosign_engine::EffectSpec::SendMutatingCommand,
        carosign_engine::TransitionIndex::new(1),
        0,
    );
    assert_eq!(
        stage_arm(&mut *tx, &arm, effect),
        Err(StorageError::GenerationConflict)
    );
    tx.abort();
}

#[test]
fn outbox_v3_checks_full_key_and_matches_official_schema() {
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    use carosign_protocol::carosign::storage::v1 as official;
    let mutation = sample();
    for phase in [
        EffectPhase::Planned,
        EffectPhase::Armed,
        EffectPhase::Observed,
        EffectPhase::ReconciliationRequired,
    ] {
        let record = EffectOutboxRecord {
            key: EffectKey {
                operation: mutation.operation,
                transition_index: TransitionIndex::new(0x0102030405060708),
                position: EffectPosition::new(0x01020304),
            },
            command: Some(CommandId::from_bytes([9; 16]).unwrap()),
            phase,
            mutation: Some(mutation.mutation),
        };
        let bytes = encode_outbox(&record).unwrap();
        let key = record.storage_key();
        assert_eq!(&key[16..24], &0x0102030405060708u64.to_be_bytes());
        assert_eq!(&key[24..], &0x01020304u32.to_be_bytes());
        assert_eq!(decode_outbox(&key, &bytes), Ok(record.clone()));
        for offset in [0, 16, 24] {
            let mut foreign = key;
            foreign[offset] ^= 1;
            assert!(decode_outbox(&foreign, &bytes).is_err());
        }
        let payload = crate::enrollment_codec::unenvelope_with(&bytes, sha256).unwrap();
        let value = official::StoredValue::decode(payload.as_slice()).unwrap();
        assert_eq!(value.schema_version, 3);
        let Some(official::stored_value::Value::Outbox(outbox)) = value.value else {
            panic!("нет очереди")
        };
        assert_eq!(outbox.operation_id, mutation.operation.as_bytes());
        assert_eq!(outbox.transition_index, record.key.transition_index.value());
        assert_eq!(outbox.effect_position, record.key.position.value());
    }
}

#[test]
fn mutation_outbox_requires_matching_operation_mutation_and_command() {
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    let mut mutation = sample();
    mutation.phase = MutationPhase::DispatchArmed;
    mutation.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    mutation.epoch = Some(DeviceEpoch::FIRST);
    mutation.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    let valid = EffectOutboxRecord {
        key: EffectKey {
            operation: mutation.operation,
            transition_index: TransitionIndex::new(2),
            position: EffectPosition::new(0),
        },
        command: mutation.command,
        phase: EffectPhase::Armed,
        mutation: Some(mutation.mutation),
    };
    assert!(validate_mutation_outbox(&mutation, &valid).is_ok());
    for invalid in [
        EffectOutboxRecord {
            key: EffectKey {
                operation: OperationId::from_bytes([11; 16]).unwrap(),
                ..valid.key
            },
            ..valid.clone()
        },
        EffectOutboxRecord {
            mutation: Some(MutationId::from_bytes([12; 16]).unwrap()),
            ..valid.clone()
        },
        EffectOutboxRecord {
            command: Some(CommandId::from_bytes([13; 16]).unwrap()),
            ..valid.clone()
        },
        EffectOutboxRecord {
            command: None,
            ..valid.clone()
        },
        EffectOutboxRecord {
            phase: EffectPhase::Planned,
            ..valid.clone()
        },
        EffectOutboxRecord {
            phase: EffectPhase::Observed,
            ..valid.clone()
        },
    ] {
        assert!(validate_mutation_outbox(&mutation, &invalid).is_err());
    }
    mutation.phase = MutationPhase::ProviderObserved;
    mutation.provider_observation_digest = Some([14; 32]);
    assert!(validate_mutation_outbox(&mutation, &valid).is_err());
    assert!(
        validate_mutation_outbox(
            &mutation,
            &EffectOutboxRecord {
                phase: EffectPhase::Observed,
                ..valid.clone()
            }
        )
        .is_ok()
    );
    mutation.phase = MutationPhase::Prepared;
    mutation.provider_observation_digest = None;
    assert!(validate_mutation_outbox(&mutation, &valid).is_err());
    assert!(!crate::recovery::may_replay(&mutation));
}

#[test]
fn startup_v3_checks_journal_lock_and_outbox_without_replay() {
    use crate::recovery::{StartupScan, may_replay};
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};

    let store = MemoryStore::default();
    let mut record = sample();
    let mut tx = store.begin_write().unwrap();
    put_sample_idempotency(&mut *tx, &record);
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        StartupScan::scan_v3(&store).unwrap().safe_to_close,
        vec![record.mutation]
    );

    record.phase = MutationPhase::DispatchArmed;
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    record.epoch = Some(DeviceEpoch::FIRST);
    record.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    let mut outbox = EffectOutboxRecord {
        key: EffectKey {
            operation: record.operation,
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: record.command,
        phase: EffectPhase::Armed,
        mutation: Some(record.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EffectOutbox,
        &outbox.storage_key(),
        &encode_outbox(&outbox).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        StartupScan::scan_v3(&store).unwrap().needing_reconciliation,
        vec![record.mutation]
    );
    assert!(!may_replay(&record));

    // The same command number at two addresses is not two independent changes; the log saves
    // exactly one submission.
    let mut duplicate = outbox.clone();
    duplicate.key.operation = OperationId::from_bytes([77; 16]).unwrap();
    duplicate.key.position = EffectPosition::new(1);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EffectOutbox,
        &duplicate.storage_key(),
        &encode_outbox(&duplicate).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, record.mutation, record.operation)
            .is_err()
    );
    let observation = ObservedMutation {
        mutation: record.mutation,
        operation: record.operation,
        command: record.command.unwrap(),
        worker: record.worker.unwrap(),
        epoch: record.epoch.unwrap(),
        effect: outbox.key,
        observation_digest: [11; 32],
        recovery_ref: None,
    };
    let mut tx = store.begin_write().unwrap();
    assert!(stage_observation(&mut *tx, &observation).is_err());
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::EffectOutbox, &duplicate.storage_key())
        .unwrap();
    tx.commit_durable().unwrap();

    let mut conflicting = record.clone();
    conflicting.mutation = MutationId::from_bytes([76; 16]).unwrap();
    for operation in [record.operation, OperationId::from_bytes([75; 16]).unwrap()] {
        conflicting.operation = operation;
        let mut tx = store.begin_write().unwrap();
        tx.put(
            Table::Mutations,
            conflicting.mutation.as_bytes(),
            &encode_mutation(&conflicting).unwrap(),
        )
        .unwrap();
        tx.commit_durable().unwrap();
        assert!(StartupScan::scan_v3(&store).is_err());
        assert!(
            crate::recovery::read_mutation_snapshot_v3(&store, record.mutation, record.operation,)
                .is_err()
        );
    }
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::Mutations, conflicting.mutation.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();

    outbox.phase = EffectPhase::Planned;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EffectOutbox,
        &outbox.storage_key(),
        &encode_outbox(&outbox).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
    outbox.phase = EffectPhase::Armed;

    outbox.command = Some(CommandId::from_bytes([10; 16]).unwrap());
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EffectOutbox,
        &outbox.storage_key(),
        &encode_outbox(&outbox).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());

    outbox.command = record.command;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EffectOutbox,
        &outbox.storage_key(),
        &encode_outbox(&outbox).unwrap(),
    )
    .unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
}

#[test]
fn uncertain_transition_is_atomic_and_never_overwrites_committed_phase() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    let store = MemoryStore::default();
    let mut record = sample();
    record.phase = MutationPhase::DispatchArmed;
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    record.epoch = Some(DeviceEpoch::FIRST);
    record.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    let effect = EffectOutboxRecord {
        key: EffectKey {
            operation: record.operation,
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: record.command,
        phase: EffectPhase::Armed,
        mutation: Some(record.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    tx.put(
        Table::EffectOutbox,
        &effect.storage_key(),
        &encode_outbox(&effect).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();

    *store.fail_next_commit.lock().unwrap() = true;
    assert_eq!(
        persist_needs_reconciliation(&store, record.mutation, record.operation),
        Err(StorageError::Unavailable)
    );
    let read = store.begin_read().unwrap();
    assert_eq!(
        decode_mutation(
            &read
                .get(Table::Mutations, record.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::DispatchArmed
    );
    assert_eq!(
        decode_outbox(
            &effect.storage_key(),
            &read
                .get(Table::EffectOutbox, &effect.storage_key())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        EffectPhase::Armed
    );
    drop(read);

    persist_needs_reconciliation(&store, record.mutation, record.operation).unwrap();
    let read = store.begin_read().unwrap();
    assert_eq!(
        decode_mutation(
            &read
                .get(Table::Mutations, record.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::NeedsReconciliation
    );
    assert_eq!(
        decode_outbox(
            &effect.storage_key(),
            &read
                .get(Table::EffectOutbox, &effect.storage_key())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        EffectPhase::ReconciliationRequired
    );
    assert_eq!(
        read.get(Table::MutationScopeLock, &record.scope_key())
            .unwrap()
            .as_deref(),
        Some(record.mutation.as_bytes().as_slice())
    );
    drop(read);

    record.phase = MutationPhase::RegistryCommitted;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    assert_eq!(
        stage_needs_reconciliation(&mut *tx, record.mutation, record.operation),
        Err(StorageError::GenerationConflict)
    );
    tx.abort();
}

#[test]
fn uncertain_transition_survives_redb_reopen() {
    use crate::redb::backend::RedbStore;
    use crate::store::MetadataStore;
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};

    let path =
        std::env::temp_dir().join(format!("carosign-uncertain-{}.redb", std::process::id()));
    let store = RedbStore::create(&path).unwrap();
    let mut record = sample();
    record.phase = MutationPhase::DispatchArmed;
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    record.epoch = Some(DeviceEpoch::FIRST);
    record.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    let effect = EffectOutboxRecord {
        key: EffectKey {
            operation: record.operation,
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: record.command,
        phase: EffectPhase::Armed,
        mutation: Some(record.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    put_sample_idempotency(&mut *tx, &record);
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    tx.put(
        Table::EffectOutbox,
        &effect.storage_key(),
        &encode_outbox(&effect).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    persist_needs_reconciliation(&store, record.mutation, record.operation).unwrap();
    drop(store);

    let reopened = RedbStore::open(&path).unwrap();
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&reopened, record.mutation, record.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::NeedsReconciliation);
    assert_eq!(
        snapshot.outbox[0].phase,
        EffectPhase::ReconciliationRequired
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn startup_v3_keeps_no_change_locked_and_rejects_completed_lock() {
    use crate::recovery::StartupScan;
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    let store = MemoryStore::default();
    let mut record = sample();
    record.phase = MutationPhase::NoChange;
    record.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    record.epoch = Some(DeviceEpoch::FIRST);
    record.command = Some(CommandId::from_bytes([9; 16]).unwrap());
    record.provider_observation_digest = Some([10; 32]);
    let outbox = EffectOutboxRecord {
        key: EffectKey {
            operation: record.operation,
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: record.command,
        phase: EffectPhase::Observed,
        mutation: Some(record.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    put_sample_idempotency(&mut *tx, &record);
    tx.put(
        Table::EffectOutbox,
        &outbox.storage_key(),
        &encode_outbox(&outbox).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let scan = StartupScan::scan_v3(&store).unwrap();
    assert_eq!(scan.needs_decision, vec![record.mutation]);
    assert!(!scan.allows_ready());
    record.phase = MutationPhase::Completed;
    record.terminal_outcome = Some(MutationTerminalOutcome::NoChange);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        StartupScan::scan_v3(&store).unwrap().closed,
        vec![record.mutation]
    );
}

#[test]
fn startup_v3_rejects_missing_or_dangling_registry_index() {
    use crate::recovery::StartupScan;
    use crate::store::{MetadataStore, Table, tests_support::MemoryStore};
    let store = MemoryStore::default();
    let sample = sample();
    let entry = crate::enrollment_codec::EnrollmentRecord {
        subject: sample.subject,
        enrollment: sample.plan.unwrap().old_target.unwrap(),
        display_label: None,
        label: "палец".into(),
        status: crate::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1]).unwrap(),
        format_id: "example".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let key = crate::redb::codec::enrollment_index_key(&entry);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        entry.enrollment.id.as_bytes(),
        &crate::enrollment_codec::encode_v3(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::EnrollmentIndex, &key, entry.enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_ok());
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::Enrollments, entry.enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan_v3(&store).is_err());
}

#[test]
fn durable_v3_arm_survives_reopen_and_is_never_replayed() {
    use crate::recovery::{StartupScan, may_replay};
    use crate::redb::backend::RedbStore;
    use crate::store::MetadataStore;
    use carosign_engine::transition::{EffectPosition, TransitionIndex};
    use carosign_engine::{EffectSpec, PlannedEffect};

    let sample = sample();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "carosign-v3-recovery-{}-{nonce}.redb",
        std::process::id()
    ));
    {
        let store = RedbStore::create_v3(&path).unwrap();
        let mut tx = store.begin_write().unwrap();
        assert_eq!(
            stage_prepared(&mut *tx, &input, &request),
            Ok(Reservation::Staged)
        );
        tx.commit_durable().unwrap();
        let arm = ArmMutation {
            mutation: input.mutation,
            worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
            epoch: DeviceEpoch::FIRST,
            command: CommandId::from_bytes([9; 16]).unwrap(),
            recovery_ref: None,
        };
        let effect = PlannedEffect {
            transition: TransitionIndex::new(1),
            position: EffectPosition::new(0).value(),
            spec: EffectSpec::SendMutatingCommand,
        };
        let plan = input
            .plan
            .to_core(Scope {
                subject: input.subject,
                authenticator: input.authenticator,
            })
            .unwrap();
        let ticket = arm_durable_v3(&store, input.operation, &arm, effect, &plan).unwrap();
        assert_eq!(ticket.operation(), input.operation);
        assert_eq!(ticket.effect(), Some(effect));
    }
    {
        let store = RedbStore::open_v3_guarded(&path).unwrap();
        let scan = StartupScan::scan_v3(&store).unwrap();
        assert_eq!(scan.needing_reconciliation, vec![input.mutation]);
        let tx = store.begin_read().unwrap();
        let record = decode_mutation(
            &tx.get(Table::Mutations, input.mutation.as_bytes())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert!(!may_replay(&record));
        drop(tx);
        let plan = input
            .plan
            .to_core(Scope {
                subject: input.subject,
                authenticator: input.authenticator,
            })
            .unwrap();
        let repeated = ArmMutation {
            mutation: input.mutation,
            worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
            epoch: DeviceEpoch::FIRST,
            command: CommandId::from_bytes([9; 16]).unwrap(),
            recovery_ref: None,
        };
        let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0);
        assert!(matches!(
            arm_durable_v3(&store, input.operation, &repeated, effect, &plan),
            Err(DurableArmFailure::BeforeCommit(
                StorageError::GenerationConflict
            ))
        ));
    }
    {
        let store = RedbStore::open(&path).unwrap();
        let mut tx = store.begin_write().unwrap();
        stage_observation(
            &mut *tx,
            &ObservedMutation {
                mutation: input.mutation,
                operation: input.operation,
                worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
                epoch: DeviceEpoch::FIRST,
                command: CommandId::from_bytes([9; 16]).unwrap(),
                effect: carosign_engine::transition::EffectKey {
                    operation: input.operation,
                    transition_index: TransitionIndex::new(1),
                    position: EffectPosition::new(0),
                },
                observation_digest: [11; 32],
                recovery_ref: Some(OpaqueProviderRef::new(vec![12]).unwrap()),
            },
        )
        .unwrap();
        tx.commit_durable().unwrap();
    }
    {
        let store = RedbStore::open(&path).unwrap();
        assert_eq!(
            StartupScan::scan_v3(&store).unwrap().needing_reconciliation,
            vec![input.mutation]
        );
        let tx = store.begin_read().unwrap();
        let record = decode_mutation(
            &tx.get(Table::Mutations, input.mutation.as_bytes())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(record.phase, MutationPhase::ProviderObserved);
        assert_eq!(record.provider_observation_digest, Some([11; 32]));
        assert_eq!(
            tx.get(Table::MutationScopeLock, &record.scope_key())
                .unwrap()
                .as_deref(),
            Some(input.mutation.as_bytes().as_slice())
        );
        let key = crate::redb::codec::outbox_key(input.operation.as_bytes(), 1, 0);
        assert_eq!(
            decode_outbox(&key, &tx.get(Table::EffectOutbox, &key).unwrap().unwrap())
                .unwrap()
                .phase,
            EffectPhase::Observed
        );
        assert!(!may_replay(&record));
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn outbox_refuses_unknown_phase_zero_transition_and_invalid_ids() {
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    let mut record = EffectOutboxRecord {
        key: EffectKey {
            operation: sample().operation,
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: None,
        phase: EffectPhase::Planned,
        mutation: None,
    };
    let key = record.storage_key();
    let bytes = encode_outbox(&record).unwrap();
    let payload = crate::enrollment_codec::unenvelope_with(&bytes, sha256).unwrap();
    let mut stored = wire::StoredValue::decode(payload.as_slice()).unwrap();
    let Some(wire::stored_value::Value::Outbox(value)) = &mut stored.value else {
        unreachable!()
    };
    value.phase = 99;
    assert!(
        decode_outbox(
            &key,
            &crate::enrollment_codec::envelope_with(&stored.encode_to_vec(), sha256).unwrap()
        )
        .is_err()
    );
    record.key.transition_index = TransitionIndex::INITIAL;
    assert!(encode_outbox(&record).is_err());
}

#[test]
fn idempotency_v3_checks_key_fields_and_authoritative_wire() {
    use carosign_protocol::carosign::storage::v1 as official;
    let record = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: Some(MutationId::from_bytes([5; 16]).unwrap()),
    };
    let key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    assert_eq!(&key[..32], &record.caller_digest);
    assert_eq!(&key[32..], record.key.as_bytes());
    let encoded = encode_idempotency(&record).unwrap();
    assert_eq!(decode_idempotency(&key, &encoded), Ok(record.clone()));
    let payload = crate::enrollment_codec::unenvelope_with(&encoded, sha256).unwrap();
    let official = official::StoredValue::decode(payload.as_slice()).unwrap();
    let Some(official::stored_value::Value::Idempotency(value)) = official.value else {
        panic!("нет записи")
    };
    assert_eq!(value.normalized_request_digest, record.request_digest);
    let mut foreign = key;
    foreign[31] ^= 1;
    assert!(decode_idempotency(&foreign, &encoded).is_err());
    let mut parsed = wire::StoredValue::decode(payload.as_slice()).unwrap();
    let Some(wire::stored_value::Value::Idempotency(value)) = &mut parsed.value else {
        unreachable!()
    };
    value.normalized_request_digest.pop();
    let invalid = crate::enrollment_codec::envelope_with(&parsed.encode_to_vec(), sha256).unwrap();
    assert!(decode_idempotency(&key, &invalid).is_err());
    assert_eq!(format!("{record:?}"), "IdempotencyRecord(..)");
}

#[test]
fn v4_pre_dispatch_no_change_survives_reopen_with_scope_blocked() {
    use crate::store::{MetadataStore, Table};
    let sample = sample();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let dir = std::env::temp_dir().join(format!("carosign-v4-no-change-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("store.redb");
    let store = crate::redb::backend::RedbStore::create_current(&path).unwrap();
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_prepared_v4(&mut *tx, &prepare, &request).unwrap();
    tx.commit_durable().unwrap();
    let snapshot =
        crate::mutation_store::inspect_prepared_v4(&store, prepare.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::Prepared);
    assert!(snapshot.outbox.is_empty());
    assert!(
        crate::mutation_store::inspect_prepared_v4(
            &store,
            prepare.mutation,
            OperationId::from_bytes([99; 16]).unwrap()
        )
        .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_pre_dispatch_no_change_v4(
        &mut *tx,
        prepare.mutation,
        prepare.operation,
    )
    .unwrap();
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_pre_dispatch_no_change_v4(
        &mut *tx,
        prepare.mutation,
        prepare.operation,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    assert!(
        crate::mutation_store::inspect_prepared_v4(&store, prepare.mutation, prepare.operation)
            .is_err()
    );
    let tx = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*tx).unwrap();
    assert_eq!(scan.scan.needs_decision, vec![prepare.mutation]);
    assert_eq!(scan.blocked.len(), 1);
    assert_eq!(scan.blocked[0].mutation, prepare.mutation);
    assert!(tx.scan_prefix(Table::EffectOutbox, &[]).unwrap().is_empty());
    drop(tx);
    let mut tx = store.begin_write().unwrap();
    assert!(
        crate::mutation_store::stage_pre_dispatch_no_change_v4(
            &mut *tx,
            prepare.mutation,
            prepare.operation
        )
        .is_err()
    );
    tx.abort();
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn v4_fresh_preparation_issues_single_durable_ticket() {
    use crate::store::MetadataStore;
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    let sample = sample();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let dir = std::env::temp_dir().join(format!("carosign-v4-fresh-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("store.redb");
    let store = crate::redb::backend::RedbStore::create_current(&path).unwrap();
    let fresh = crate::mutation_store::prepare_fresh_arm_v4(&store, &input, &request).unwrap();
    assert!(matches!(
        crate::mutation_store::prepare_fresh_arm_v4(&store, &input, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    let arm = ArmMutation {
        mutation: input.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let _ticket = fresh.arm(&arm, effect).unwrap();
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let tx = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*tx).unwrap();
    assert_eq!(scan.scan.needing_reconciliation, vec![arm.mutation]);
    drop(tx);
    assert!(matches!(
        crate::mutation_store::prepare_fresh_arm_v4(&store, &input, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn v4_fresh_prepare_lost_commit_ack_does_not_issue_capability() {
    use disk_lost_ack::LostAck;

    let sample = sample();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let dir = std::env::temp_dir().join(format!("carosign-v4-fresh-lost-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("store.redb");
    let store = crate::redb::backend::RedbStore::create_current(&path).unwrap();
    assert!(matches!(
        crate::mutation_store::prepare_fresh_arm_v4(&LostAck(&store), &input, &request),
        Err(FreshPreparationFailure::CommitUnknown(
            StorageError::Unavailable
        ))
    ));
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    assert!(matches!(
        crate::mutation_store::prepare_fresh_arm_v4(&store, &input, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn v4_fresh_arm_lost_commit_ack_keeps_scope_blocked() {
    use crate::store::MetadataStore;
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    use std::sync::atomic::{AtomicBool, Ordering};

    use lost_arm_ack::LostArmAck;

    let sample = sample();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let dir = std::env::temp_dir().join(format!("carosign-v4-arm-lost-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("store.redb");
    let store = crate::redb::backend::RedbStore::create_current(&path).unwrap();
    let wrapper = LostArmAck {
        store: &store,
        lose_ack: AtomicBool::new(false),
    };
    let fresh = crate::mutation_store::prepare_fresh_arm_v4(&wrapper, &input, &request).unwrap();
    wrapper.lose_ack.store(true, Ordering::SeqCst);
    let arm = ArmMutation {
        mutation: input.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    assert!(matches!(
        fresh.arm(&arm, effect),
        Err(DurableArmFailure::CommitUnknown(StorageError::Unavailable))
    ));
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let tx = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*tx).unwrap();
    assert_eq!(scan.scan.needing_reconciliation, vec![arm.mutation]);
    assert_eq!(scan.blocked.len(), 1);
    drop(tx);
    assert!(matches!(
        crate::mutation_store::prepare_fresh_arm_v4(&store, &input, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn v4_registry_commit_and_completion_are_atomic_and_scoped() {
    use crate::store::MetadataStore;
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    let sample = sample();
    let store = crate::test_support::MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::mutation_store::SCHEMA_V4,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let new_id = EnrollmentId::from_bytes([13; 16]).unwrap();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(new_id),
            1,
            vec![],
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .and_then(|plan| {
            plan.with_requested_label(
                Scope {
                    subject: sample.subject,
                    authenticator: sample.authenticator,
                },
                "test",
            )
        })
        .unwrap(),
    };
    let idem = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: crate::mutation_plan::canonical_request_digest(
            Scope {
                subject: prepare.subject,
                authenticator: prepare.authenticator,
            },
            Transition::EnrollNew,
            None,
            Some("test"),
        )
        .unwrap(),
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_prepared_v4(&mut *tx, &prepare, &idem).unwrap();
    tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    let bytes = read
        .get(Table::Mutations, prepare.mutation.as_bytes())
        .unwrap()
        .unwrap();
    let v3 = crate::installation_records::checked_v4_record_as_v3(
        crate::installation_records::MigratableRecord::Mutation,
        prepare.mutation.as_bytes(),
        &bytes,
    )
    .unwrap();
    let saved = decode_mutation(&v3).unwrap().plan.unwrap();
    assert_eq!(saved.requested_label.as_deref(), Some("test"));
    let mut tampered = saved.clone();
    tampered.requested_label = Some("other".into());
    assert!(
        tampered
            .validate(Scope {
                subject: prepare.subject,
                authenticator: prepare.authenticator
            })
            .is_err()
    );
    drop(read);
    let arm = ArmMutation {
        mutation: prepare.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_arm_v4(&mut *tx, &arm, effect).unwrap();
    tx.commit_durable().unwrap();
    let observation = ObservedMutation {
        mutation: prepare.mutation,
        operation: prepare.operation,
        command: arm.command,
        worker: arm.worker,
        epoch: arm.epoch,
        effect: carosign_engine::transition::EffectKey {
            operation: prepare.operation,
            transition_index: effect.transition,
            position: carosign_engine::transition::EffectPosition::new(effect.position),
        },
        observation_digest: [11; 32],
        recovery_ref: None,
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_observation_v4(&mut *tx, &observation).unwrap();
    tx.commit_durable().unwrap();
    let new = crate::enrollment_codec::EnrollmentRecord {
        subject: prepare.subject,
        enrollment: EnrollmentRef {
            id: new_id,
            authenticator: prepare.authenticator,
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "test".into(),
        status: crate::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![12]).unwrap(),
        format_id: "mock".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let update = RegistryUpdate {
        mutation: prepare.mutation,
        operation: prepare.operation,
        command: arm.command,
        worker: arm.worker,
        epoch: arm.epoch,
        observation_digest: [11; 32],
        new_record: Some(&new),
    };
    let mut foreign_label = new.clone();
    foreign_label.label = "other".into();
    let wrong = RegistryUpdate {
        new_record: Some(&foreign_label),
        ..update
    };
    let mut tx = store.begin_write().unwrap();
    assert!(crate::mutation_store::stage_registry_update_v4(&mut *tx, &wrong).is_err());
    tx.abort();
    let update = RegistryUpdate {
        new_record: Some(&new),
        ..wrong
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_registry_update_v4(&mut *tx, &update).unwrap();
    tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    assert!(
        read.get(Table::Enrollments, new_id.as_bytes())
            .unwrap()
            .is_some()
    );
    assert!(
        read.get(Table::MutationScopeLock, &sample.scope_key())
            .unwrap()
            .is_some()
    );
    crate::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
    drop(read);
    let mut tx = store.begin_write().unwrap();
    assert!(crate::mutation_store::stage_registry_update_v4(&mut *tx, &update).is_err());
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_completed_v4(&mut *tx, prepare.mutation, prepare.operation)
        .unwrap();
    tx.abort();
    let read = store.begin_read().unwrap();
    assert!(
        read.get(Table::MutationScopeLock, &sample.scope_key())
            .unwrap()
            .is_some()
    );
    drop(read);
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_completed_v4(&mut *tx, prepare.mutation, prepare.operation)
        .unwrap();
    tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    assert!(
        read.get(Table::MutationScopeLock, &sample.scope_key())
            .unwrap()
            .is_none()
    );
    crate::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
    drop(read);
    let removal = PrepareMutation {
        mutation: MutationId::from_bytes([31; 16]).unwrap(),
        operation: OperationId::from_bytes([32; 16]).unwrap(),
        subject: new.subject,
        authenticator: new.enrollment.authenticator,
        expected_generation: Some(new.enrollment.generation),
        enroll: false,
        plan: MutationPlan::build(
            Transition::Remove,
            Some(new.enrollment),
            None,
            0,
            vec![BindingSnapshot {
                enrollment: new.enrollment,
                subject: new.subject,
                kind: new.binding_kind,
                provider_ref_digest: sha256(new.provider_ref.as_bytes()),
                format_id: new.format_id.clone(),
                format_version: new.format_version,
            }],
            Scope {
                subject: new.subject,
                authenticator: new.enrollment.authenticator,
            },
        )
        .unwrap(),
    };
    let idem_remove = IdempotencyRecord {
        caller_digest: [31; 32],
        key: IdempotencyKey::from_bytes([32; 16]).unwrap(),
        request_digest: [33; 32],
        operation: removal.operation,
        mutation: Some(removal.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_prepared_v4(&mut *tx, &removal, &idem_remove).unwrap();
    tx.commit_durable().unwrap();
    let remove_arm = ArmMutation {
        mutation: removal.mutation,
        worker: arm.worker,
        epoch: arm.epoch,
        command: CommandId::from_bytes([34; 16]).unwrap(),
        recovery_ref: None,
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_arm_v4(&mut *tx, &remove_arm, effect).unwrap();
    tx.commit_durable().unwrap();
    let remove_observed = ObservedMutation {
        mutation: removal.mutation,
        operation: removal.operation,
        command: remove_arm.command,
        worker: remove_arm.worker,
        epoch: remove_arm.epoch,
        effect: carosign_engine::transition::EffectKey {
            operation: removal.operation,
            transition_index: effect.transition,
            position: carosign_engine::transition::EffectPosition::new(effect.position),
        },
        observation_digest: [35; 32],
        recovery_ref: None,
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_observation_v4(&mut *tx, &remove_observed).unwrap();
    tx.commit_durable().unwrap();
    let remove_update = RegistryUpdate {
        mutation: removal.mutation,
        operation: removal.operation,
        command: remove_arm.command,
        worker: remove_arm.worker,
        epoch: remove_arm.epoch,
        observation_digest: [35; 32],
        new_record: None,
    };
    crate::mutation_store::commit_registry_v4(&store, &remove_update).unwrap();
    let read = store.begin_read().unwrap();
    assert!(
        read.get(Table::Enrollments, new_id.as_bytes())
            .unwrap()
            .is_none()
    );
    assert!(
        read.get(
            Table::EnrollmentIndex,
            &crate::redb::codec::enrollment_index_key(&new)
        )
        .unwrap()
        .is_none()
    );
    assert!(
        read.get(Table::MutationScopeLock, &sample.scope_key())
            .unwrap()
            .is_some()
    );
    drop(read);
    crate::mutation_store::complete_v4(&store, removal.mutation, removal.operation).unwrap();
    let completed =
        crate::mutation_store::read_completed_v4(&store, removal.mutation, removal.operation)
            .unwrap();
    assert_eq!(
        completed.terminal_outcome,
        Some(MutationTerminalOutcome::Committed)
    );
    assert!(
        crate::mutation_store::read_completed_v4(&store, removal.mutation, prepare.operation)
            .is_err()
    );
    let read = store.begin_read().unwrap();
    crate::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
}

#[test]
fn v4_arm_is_atomic_and_cannot_be_rearmed_after_commit() {
    use crate::store::{MetadataStore, Table};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    let sample = sample();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .and_then(|plan| {
            plan.with_requested_label(
                Scope {
                    subject: sample.subject,
                    authenticator: sample.authenticator,
                },
                "test",
            )
        })
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: crate::mutation_plan::canonical_request_digest(
            Scope {
                subject: prepare.subject,
                authenticator: prepare.authenticator,
            },
            Transition::EnrollNew,
            None,
            Some("test"),
        )
        .unwrap(),
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let dir = std::env::temp_dir().join(format!("carosign-v4-arm-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("store.redb");
    let store = crate::redb::backend::RedbStore::create_current(&path).unwrap();
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_prepared_v4(&mut *tx, &prepare, &request).unwrap();
    tx.commit_durable().unwrap();
    let arm = ArmMutation {
        mutation: prepare.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_arm_v4(&mut *tx, &arm, effect).unwrap();
    tx.abort();
    let tx = store.begin_read().unwrap();
    assert!(tx.scan_prefix(Table::EffectOutbox, &[]).unwrap().is_empty());
    drop(tx);
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_arm_v4(&mut *tx, &arm, effect).unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let tx = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*tx).unwrap();
    assert_eq!(scan.scan.needing_reconciliation, vec![arm.mutation]);
    assert_eq!(scan.blocked.len(), 1);
    assert_eq!(scan.blocked[0].mutation, arm.mutation);
    let bytes = tx
        .get(Table::Mutations, arm.mutation.as_bytes())
        .unwrap()
        .unwrap();
    let converted = crate::installation_records::checked_v4_record_as_v3(
        crate::installation_records::MigratableRecord::Mutation,
        arm.mutation.as_bytes(),
        &bytes,
    )
    .unwrap();
    let record = decode_mutation(&converted).unwrap();
    assert_eq!(record.phase, MutationPhase::DispatchArmed);
    assert!(!crate::recovery::may_replay(&record));
    assert_eq!(tx.scan_prefix(Table::EffectOutbox, &[]).unwrap().len(), 1);
    drop(tx);
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        crate::mutation_store::stage_arm_v4(&mut *tx, &arm, effect),
        Err(StorageError::GenerationConflict)
    );
    tx.abort();
    let tx = store.begin_read().unwrap();
    let outbox_key = tx.scan_prefix(Table::EffectOutbox, &[]).unwrap()[0]
        .0
        .clone();
    let outbox_bytes = tx.get(Table::EffectOutbox, &outbox_key).unwrap().unwrap();
    let outbox_v3 = crate::installation_records::checked_v4_record_as_v3(
        crate::installation_records::MigratableRecord::Outbox,
        &outbox_key,
        &outbox_bytes,
    )
    .unwrap();
    let outbox = decode_outbox(&outbox_key, &outbox_v3).unwrap();
    drop(tx);
    let observation = ObservedMutation {
        mutation: arm.mutation,
        operation: prepare.operation,
        command: arm.command,
        worker: arm.worker,
        epoch: arm.epoch,
        effect: outbox.key,
        observation_digest: [11; 32],
        recovery_ref: None,
    };
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_observation_v4(&mut *tx, &observation).unwrap();
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_observation_v4(&mut *tx, &observation).unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let tx = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*tx).unwrap();
    assert_eq!(scan.scan.needing_reconciliation, vec![arm.mutation]);
    let record = crate::installation_records::checked_v4_record_as_v3(
        crate::installation_records::MigratableRecord::Mutation,
        arm.mutation.as_bytes(),
        &tx.get(Table::Mutations, arm.mutation.as_bytes())
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        decode_mutation(&record).unwrap().phase,
        MutationPhase::ProviderObserved
    );
    drop(tx);
    // Digest and full worker binding are mandatory even if the target entry formally fits
    // the plan.
    let new = crate::enrollment_codec::EnrollmentRecord {
        subject: prepare.subject,
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([13; 16]).unwrap(),
            authenticator: prepare.authenticator,
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "test".into(),
        status: crate::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![12]).unwrap(),
        format_id: "mock".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let mut update = RegistryUpdate {
        mutation: arm.mutation,
        operation: prepare.operation,
        command: arm.command,
        worker: arm.worker,
        epoch: arm.epoch,
        observation_digest: [99; 32],
        new_record: Some(&new),
    };
    let mut tx = store.begin_write().unwrap();
    assert!(crate::mutation_store::stage_registry_update_v4(&mut *tx, &update).is_err());
    tx.abort();
    update.observation_digest = [11; 32];
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_registry_update_v4(&mut *tx, &update).unwrap();
    tx.abort();
    let tx = store.begin_read().unwrap();
    assert!(
        tx.get(Table::Enrollments, new.enrollment.id.as_bytes())
            .unwrap()
            .is_none()
    );
    drop(tx);
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        crate::mutation_store::stage_observation_v4(&mut *tx, &observation),
        Err(StorageError::GenerationConflict)
    );
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    crate::mutation_store::stage_needs_reconciliation_v4(&mut *tx, arm.mutation, prepare.operation)
        .unwrap();
    tx.abort();
    // The log port must use the same atomic junction v4.
    let journal = crate::journal_store::StoreJournal::new(store);
    crate::journal::MutationRepository::mark_uncertain(&journal, arm.mutation).unwrap();
    let record = journal.record(arm.mutation).unwrap();
    assert_eq!(record.phase, MutationPhase::NeedsReconciliation);
    let incomplete = crate::journal::MutationRepository::scan_incomplete(&journal).unwrap();
    assert_eq!(incomplete.len(), 1);
    assert_eq!(incomplete[0].mutation, arm.mutation);
    drop(journal);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let tx = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*tx).unwrap();
    assert_eq!(scan.scan.needing_reconciliation, vec![arm.mutation]);
    assert_eq!(scan.blocked.len(), 1);
    let record = crate::installation_records::checked_v4_record_as_v3(
        crate::installation_records::MigratableRecord::Mutation,
        arm.mutation.as_bytes(),
        &tx.get(Table::Mutations, arm.mutation.as_bytes())
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        decode_mutation(&record).unwrap().phase,
        MutationPhase::NeedsReconciliation
    );
    drop(tx);
    let plan = crate::mutation_store::read_reconciliation_plan_v4(
        &store,
        arm.mutation,
        prepare.operation,
        prepare.subject,
        prepare.authenticator,
        arm.epoch,
    )
    .unwrap();
    assert_eq!(plan.scope_digest(), &prepare.plan.scope_digest);
    assert!(
        crate::mutation_store::read_reconciliation_plan_v4(
            &store,
            arm.mutation,
            prepare.operation,
            prepare.subject,
            prepare.authenticator,
            DeviceEpoch::new(arm.epoch.value() + 1).unwrap(),
        )
        .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::EffectOutbox, &outbox_key).unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::mutation_store::read_reconciliation_plan_v4(
            &store,
            arm.mutation,
            prepare.operation,
            prepare.subject,
            prepare.authenticator,
            arm.epoch,
        )
        .is_err()
    );
    drop(store);
    let before = std::fs::read(&path).unwrap();
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn v4_preparation_uses_one_durable_transaction_and_checked_records() {
    use crate::store::{MetadataStore, Table};
    let sample = sample();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let dir = std::env::temp_dir().join(format!("carosign-v4-prepared-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("v4.redb");
    let store = crate::redb::backend::RedbStore::create_current(&path).unwrap();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        crate::mutation_store::stage_prepared_v4(&mut *tx, &input, &request),
        Ok(Reservation::Staged)
    );
    tx.abort();
    let read = store.begin_read().unwrap();
    assert!(read.scan_prefix(Table::Mutations, &[]).unwrap().is_empty());
    assert!(
        read.scan_prefix(Table::MutationScopeLock, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        read.scan_prefix(Table::Idempotency, &[])
            .unwrap()
            .is_empty()
    );
    drop(read);
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        crate::mutation_store::stage_prepared_v4(&mut *tx, &input, &request),
        Ok(Reservation::Staged)
    );
    tx.commit_durable().unwrap();
    drop(store);
    let store = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let read = store.begin_read().unwrap();
    let scan = crate::mutation_store::scan_prepared_v4_snapshot(&*read).unwrap();
    assert_eq!(scan.scan.safe_to_close, vec![input.mutation]);
    assert_eq!(scan.blocked.len(), 1);
    assert_eq!(scan.blocked[0].mutation, input.mutation);
    drop(read);
    let mut tx = store.begin_write().unwrap();
    let projection =
        crate::mutation_store::enrollment_projection_v4(&mut *tx, input.mutation, input.operation)
            .unwrap();
    assert!(matches!(
        projection.change,
        crate::projection::ProjectionChange::Create { planned_new }
            if Some(planned_new.id) == input.plan.new_enrollment
                && planned_new.authenticator == input.authenticator
    ));
    assert!(
        crate::mutation_store::enrollment_projection_v4(
            &mut *tx,
            input.mutation,
            OperationId::from_bytes([88; 16]).unwrap(),
        )
        .is_err()
    );
    assert!(matches!(
        crate::mutation_store::stage_prepared_v4(&mut *tx, &input, &request),
        Ok(Reservation::Existing(_))
    ));
    tx.abort();
    let read = store.begin_read().unwrap();
    let bytes = read
        .get(Table::Mutations, input.mutation.as_bytes())
        .unwrap()
        .unwrap();
    assert!(decode_mutation(&bytes).is_err());
    let converted = crate::installation_records::checked_v4_record_as_v3(
        crate::installation_records::MigratableRecord::Mutation,
        input.mutation.as_bytes(),
        &bytes,
    )
    .unwrap();
    assert_eq!(
        decode_mutation(&converted).unwrap().phase,
        MutationPhase::Prepared
    );
    drop(read);
    drop(store);
    let db = redb::Database::open(&path).unwrap();
    let tx = db.begin_write().unwrap();
    {
        use redb::TableDefinition;
        let mut locks = tx
            .open_table(TableDefinition::<&[u8], &[u8]>::new("mutation_scope_lock"))
            .unwrap();
        let key = sample.scope_key();
        locks.remove(key.as_slice()).unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn prepared_mutation_stages_journal_lock_and_idempotency_atomically() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    let sample = sample();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let store = MemoryStore::default();
    let old = IdempotencyRecord {
        caller_digest: [8; 32],
        key: IdempotencyKey::from_bytes([9; 16]).unwrap(),
        request_digest: [10; 32],
        operation: input.operation,
        mutation: None,
    };
    let old_key = crate::redb::codec::idempotency_key(&old.caller_digest, &old.key);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Idempotency,
        &old_key,
        &encode_idempotency(&old).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_prepared(&mut *tx, &input, &request),
        Err(ReservationError::Storage(StorageError::GenerationConflict))
    );
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::Idempotency, &old_key).unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_prepared(&mut *tx, &input, &request),
        Ok(Reservation::Staged)
    );
    *store.fail_next_commit.lock().unwrap() = true;
    assert_eq!(tx.commit_durable(), Err(StorageError::Unavailable));
    let read = store.begin_read().unwrap();
    for table in [
        Table::Mutations,
        Table::MutationScopeLock,
        Table::Idempotency,
    ] {
        assert!(read.scan_prefix(table, &[]).unwrap().is_empty());
    }
    drop(read);
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_prepared(&mut *tx, &input, &request),
        Ok(Reservation::Staged)
    );
    tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    let encoded = read
        .get(Table::Mutations, input.mutation.as_bytes())
        .unwrap()
        .unwrap();
    let record = decode_mutation(&encoded).unwrap();
    assert_eq!(record.phase, MutationPhase::Prepared);
    assert_eq!(
        read.get(Table::MutationScopeLock, &record.scope_key())
            .unwrap()
            .unwrap(),
        input.mutation.as_bytes()
    );
    let key = crate::redb::codec::idempotency_key(&request.caller_digest, &request.key);
    assert_eq!(
        decode_idempotency(&key, &read.get(Table::Idempotency, &key).unwrap().unwrap()),
        Ok(request.clone())
    );
    drop(read);

    let mut tx = store.begin_write().unwrap();
    let projection = enrollment_projection(&*tx, input.mutation, input.operation).unwrap();
    assert!(
        matches!(projection.change, crate::projection::ProjectionChange::Create { planned_new }
            if planned_new.id == input.plan.new_enrollment.unwrap()
                && planned_new.authenticator == input.authenticator)
    );
    assert!(
        enrollment_projection(
            &*tx,
            input.mutation,
            OperationId::from_bytes([88; 16]).unwrap()
        )
        .is_err()
    );
    let other = crate::enrollment_codec::EnrollmentRecord {
        subject: input.subject,
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([91; 16]).unwrap(),
            authenticator: input.authenticator,
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: crate::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1]).unwrap(),
        format_id: "example".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    tx.put(
        Table::Enrollments,
        other.enrollment.id.as_bytes(),
        &crate::enrollment_codec::encode_v3(&other).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        enrollment_projection(&*tx, input.mutation, input.operation),
        Err(StorageError::Corrupt { .. })
    ));
    tx.put(
        Table::EnrollmentIndex,
        &crate::redb::codec::enrollment_index_key(&other),
        other.enrollment.id.as_bytes(),
    )
    .unwrap();
    assert_eq!(
        enrollment_projection(&*tx, input.mutation, input.operation),
        Err(StorageError::GenerationConflict)
    );
    tx.delete(Table::Enrollments, other.enrollment.id.as_bytes())
        .unwrap();
    tx.delete(
        Table::EnrollmentIndex,
        &crate::redb::codec::enrollment_index_key(&other),
    )
    .unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    assert!(enrollment_projection(&*tx, input.mutation, input.operation).is_err());
    tx.abort();

    let mut retry = input.clone();
    retry.mutation = MutationId::from_bytes([14; 16]).unwrap();
    retry.operation = OperationId::from_bytes([15; 16]).unwrap();
    let repeated = IdempotencyRecord {
        operation: retry.operation,
        mutation: Some(retry.mutation),
        ..request.clone()
    };
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_prepared(&mut *tx, &retry, &repeated),
        Ok(Reservation::Existing(request.clone()))
    );
    let foreign = IdempotencyRecord {
        key: IdempotencyKey::from_bytes([31; 16]).unwrap(),
        operation: record.operation,
        mutation: None,
        ..request.clone()
    };
    let foreign_key = crate::redb::codec::idempotency_key(&foreign.caller_digest, &foreign.key);
    tx.put(
        Table::Idempotency,
        &foreign_key,
        &encode_idempotency(&foreign).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_prepared(&mut *tx, &retry, &repeated),
        Err(ReservationError::Storage(StorageError::Corrupt { .. }))
    ));
    tx.delete(Table::Idempotency, &foreign_key).unwrap();
    let bogus = EffectOutboxRecord {
        key: carosign_engine::transition::EffectKey {
            operation: record.operation,
            transition_index: carosign_engine::transition::TransitionIndex::new(1),
            position: carosign_engine::transition::EffectPosition::new(0),
        },
        command: Some(CommandId::from_bytes([22; 16]).unwrap()),
        phase: EffectPhase::Armed,
        mutation: Some(record.mutation),
    };
    tx.put(
        Table::EffectOutbox,
        &bogus.storage_key(),
        &encode_outbox(&bogus).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_prepared(&mut *tx, &retry, &repeated),
        Err(ReservationError::Storage(StorageError::Corrupt { .. }))
    ));
    tx.delete(Table::EffectOutbox, &bogus.storage_key())
        .unwrap();
    let mut completed = record.clone();
    completed.phase = MutationPhase::Completed;
    completed.terminal_outcome = Some(MutationTerminalOutcome::NoChange);
    tx.put(
        Table::Mutations,
        completed.mutation.as_bytes(),
        &encode_mutation(&completed).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_prepared(&mut *tx, &retry, &repeated),
        Err(ReservationError::Storage(StorageError::Corrupt { .. }))
    ));
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record).unwrap(),
    )
    .unwrap();
    let conflict = IdempotencyRecord {
        request_digest: [99; 32],
        ..repeated.clone()
    };
    assert_eq!(
        stage_prepared(&mut *tx, &retry, &conflict),
        Err(ReservationError::Conflict)
    );
    let other_key = IdempotencyRecord {
        key: IdempotencyKey::from_bytes([16; 16]).unwrap(),
        ..repeated
    };
    assert!(matches!(
        stage_prepared(&mut *tx, &retry, &other_key),
        Err(ReservationError::Storage(StorageError::GenerationConflict))
    ));
    tx.abort();
}

#[test]
fn prepared_mutation_rejects_stale_registry_snapshot_without_staging() {
    use crate::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    let sample = sample();
    let target = sample.plan.as_ref().unwrap().old_target.unwrap();
    let enrollment = EnrollmentRecord {
        subject: sample.subject,
        enrollment: target,
        display_label: None,
        label: "палец".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1, 2]).unwrap(),
        format_id: "example".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        target.id.as_bytes(),
        &crate::enrollment_codec::encode_v3(&enrollment).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::EnrollmentIndex,
        &crate::redb::codec::enrollment_index_key(&enrollment),
        target.id.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let input = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    assert!(matches!(
        stage_prepared(&mut *tx, &input, &request),
        Err(ReservationError::Storage(StorageError::GenerationConflict))
    ));
    for table in [
        Table::Mutations,
        Table::MutationScopeLock,
        Table::Idempotency,
    ] {
        assert!(tx.scan_prefix(table, &[]).unwrap().is_empty());
    }
    tx.abort();
}

#[test]
fn prepare_rejects_unfinished_same_scope_when_lock_is_missing() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    let existing = sample();
    let input = PrepareMutation {
        mutation: MutationId::from_bytes([60; 16]).unwrap(),
        operation: OperationId::from_bytes([61; 16]).unwrap(),
        subject: existing.subject,
        authenticator: existing.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([62; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: existing.subject,
                authenticator: existing.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        existing.mutation.as_bytes(),
        &encode_mutation(&existing).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();

    let mut tx = store.begin_write().unwrap();
    assert!(matches!(
        stage_prepared(&mut *tx, &input, &request),
        Err(ReservationError::Storage(StorageError::Corrupt { .. }))
    ));
    assert!(
        tx.get(Table::Mutations, input.mutation.as_bytes())
            .unwrap()
            .is_none()
    );
    assert!(
        tx.scan_prefix(Table::MutationScopeLock, &[])
            .unwrap()
            .is_empty()
    );
    assert!(tx.scan_prefix(Table::Idempotency, &[]).unwrap().is_empty());
    tx.abort();
}

#[test]
fn projection_and_arm_refuse_scoped_vault_without_approved_port() {
    use crate::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};

    let original = sample();
    let old = original.plan.as_ref().unwrap().old_target.unwrap();
    let entry = EnrollmentRecord {
        subject: original.subject,
        enrollment: old,
        display_label: None,
        label: "палец".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(Vec::new()).unwrap(),
        format_id: "example".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ScopedVault,
    };
    let plan = MutationPlan::build(
        Transition::EnrollReplace,
        Some(old),
        Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
        1,
        vec![BindingSnapshot {
            enrollment: old,
            subject: original.subject,
            kind: StoredBindingKind::ScopedVault,
            provider_ref_digest: sha256(&[]),
            format_id: "example".into(),
            format_version: 1,
        }],
        Scope {
            subject: original.subject,
            authenticator: original.authenticator,
        },
    )
    .unwrap();
    let input = PrepareMutation {
        mutation: original.mutation,
        operation: original.operation,
        subject: original.subject,
        authenticator: original.authenticator,
        expected_generation: Some(old.generation),
        enroll: true,
        plan,
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        old.id.as_bytes(),
        &crate::enrollment_codec::encode_v3(&entry).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::EnrollmentIndex,
        &crate::redb::codec::enrollment_index_key(&entry),
        old.id.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_prepared(&mut *tx, &input, &request),
        Ok(Reservation::Staged)
    );
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        enrollment_projection(&*tx, input.mutation, input.operation),
        Err(StorageError::Unavailable)
    );
    let arm = ArmMutation {
        mutation: input.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    assert_eq!(
        stage_arm(
            &mut *tx,
            &arm,
            PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0)
        ),
        Err(StorageError::Unavailable)
    );
    assert!(tx.scan_prefix(Table::EffectOutbox, &[]).unwrap().is_empty());
    tx.abort();
}

#[test]
fn durable_arm_issues_ticket_once_and_only_after_confirmed_commit() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    let sample = sample();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    stage_prepared(&mut *tx, &prepare, &request).unwrap();
    tx.commit_durable().unwrap();
    let input = ArmMutation {
        mutation: prepare.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let expected_plan = prepare
        .plan
        .to_core(Scope {
            subject: prepare.subject,
            authenticator: prepare.authenticator,
        })
        .unwrap();
    assert!(arm_durable_v3(&store, prepare.operation, &input, effect, &expected_plan).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Meta, b"unknown-migration-state", b"pending")
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        arm_durable_v3(&store, prepare.operation, &input, effect, &expected_plan),
        Err(DurableArmFailure::BeforeCommit(
            StorageError::Corrupt { .. }
        ))
    ));
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::Prepared);
    assert!(snapshot.outbox.is_empty());

    // The capability relates to a particular vault, even if the other contains the same
    // identifiers and plan, and a successful arm of one base should not arm or disclose a
    // ticket to another.
    let first = MemoryStore::default();
    let second = MemoryStore::default();
    for database in [&first, &second] {
        let mut tx = database.begin_write().unwrap();
        tx.put(
            Table::Meta,
            crate::redb::backend::SCHEMA_KEY,
            crate::redb::backend::SCHEMA_V3,
        )
        .unwrap();
        tx.commit_durable().unwrap();
    }
    let first_capability = prepare_fresh_arm(&first, &prepare, &request).unwrap();
    let second_capability = prepare_fresh_arm(&second, &prepare, &request).unwrap();
    first_capability.arm(&input, effect).unwrap();
    let first_snapshot =
        crate::recovery::read_mutation_snapshot_v3(&first, input.mutation, prepare.operation)
            .unwrap();
    let second_snapshot =
        crate::recovery::read_mutation_snapshot_v3(&second, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(first_snapshot.mutation.phase, MutationPhase::DispatchArmed);
    assert_eq!(second_snapshot.mutation.phase, MutationPhase::Prepared);
    assert!(second_snapshot.outbox.is_empty());
    // The loss of the second holder does not allow the possibility of the old idempotence to
    // be restored in the same process.
    drop(second_capability);
    assert!(matches!(
        prepare_fresh_arm(&second, &prepare, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::Meta, b"unknown-migration-state").unwrap();
    tx.commit_durable().unwrap();
    let foreign = OperationId::from_bytes([99; 16]).unwrap();
    assert!(arm_durable_v3(&store, foreign, &input, effect, &expected_plan).is_err());
    let foreign_plan = carosign_core::operation::plan::MutationPlan::new(
        prepare.subject,
        prepare.authenticator,
        carosign_core::operation::plan::PlanTransition::Create {
            planned_new: carosign_core::enrollment::EnrollmentRef {
                id: EnrollmentId::from_bytes([99; 16]).unwrap(),
                authenticator: prepare.authenticator,
                generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
            },
        },
        Vec::new(),
        *expected_plan.scope_digest(),
    )
    .unwrap();
    assert!(matches!(
        arm_durable_v3(&store, prepare.operation, &input, effect, &foreign_plan),
        Err(DurableArmFailure::BeforeCommit(
            StorageError::GenerationConflict
        ))
    ));
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::Prepared);
    assert!(snapshot.outbox.is_empty());
    *store.fail_next_commit.lock().unwrap() = true;
    let attempts = ArmAttemptOwner::new(1);
    assert!(matches!(
        attempts.arm(&store, prepare.operation, &input, effect, &expected_plan),
        Err(ArmAttemptFailure::Durable(
            DurableArmFailure::CommitUnknown(StorageError::Unavailable)
        ))
    ));
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::Prepared);
    assert!(snapshot.outbox.is_empty());
    assert!(matches!(
        attempts.arm(&store, prepare.operation, &input, effect, &expected_plan),
        Err(ArmAttemptFailure::AlreadyAttempted)
    ));
    // After an indefinite commit, automatic repetition is prohibited even if the fixture
    // being tested has not recorded the changes, and a successful attempt is checked on an
    // independent fixture in a separate storage.
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    stage_prepared(&mut *tx, &prepare, &request).unwrap();
    tx.commit_durable().unwrap();
    let ticket = arm_durable_v3(&store, prepare.operation, &input, effect, &expected_plan).unwrap();
    assert_eq!(ticket.operation(), prepare.operation);
    assert_eq!(ticket.command(), input.command);
    assert_eq!(ticket.worker(), input.worker);
    assert_eq!(ticket.epoch(), input.epoch);
    assert_eq!(ticket.scope(), &sample.scope_key());
    assert_eq!(ticket.effect(), Some(effect));
    assert!(matches!(
        arm_durable_v3(&store, prepare.operation, &input, effect, &expected_plan),
        Err(DurableArmFailure::BeforeCommit(
            StorageError::GenerationConflict
        ))
    ));
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::DispatchArmed);
    assert!(!crate::recovery::may_replay(&snapshot.mutation));
}

#[test]
fn only_durably_fresh_preparation_can_create_one_arm_attempt() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};

    use switchable_store::SwitchableStore;

    use lost_ack_file::LostAckFile;

    let sample = sample();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let arm = ArmMutation {
        mutation: prepare.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    tx.commit_durable().unwrap();

    // An unknown fixation of the preparation itself can leave the base without any record, a
    // state indistinguishable from the new base: this test documents why the right to repeat
    // must be held by an independent owner and why this function cannot be opened as a
    // production port on a single query field.
    let uncertain_prepare = MemoryStore::default();
    let mut tx = uncertain_prepare.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    *uncertain_prepare.fail_next_commit.lock().unwrap() = true;
    assert!(matches!(
        prepare_fresh_arm(&uncertain_prepare, &prepare, &request),
        Err(FreshPreparationFailure::CommitUnknown(
            StorageError::Unavailable
        ))
    ));
    let snapshot = uncertain_prepare.begin_read().unwrap();
    assert!(
        snapshot
            .scan_prefix(Table::Mutations, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        snapshot
            .scan_prefix(Table::Idempotency, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        snapshot
            .scan_prefix(Table::MutationScopeLock, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        snapshot
            .scan_prefix(Table::EffectOutbox, &[])
            .unwrap()
            .is_empty()
    );
    drop(snapshot);
    // Direct repetition here is not a permitted service scenario; it demonstrates the absence
    // of a discernible consistent sign of attempt.
    assert!(prepare_fresh_arm(&uncertain_prepare, &prepare, &request).is_ok());

    // The opposite outcome of the same mistake: commit did pass, but the answer was lost. The
    // owner does not get the right to arm, and the subsequent search for idempotence does not
    // create a new right from the Prepared record.
    let committed_prepare = SwitchableStore {
        inner: MemoryStore::default(),
        lose_ack: std::sync::atomic::AtomicBool::new(false),
    };
    let mut tx = committed_prepare.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    committed_prepare
        .lose_ack
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        prepare_fresh_arm(&committed_prepare, &prepare, &request),
        Err(FreshPreparationFailure::CommitUnknown(
            StorageError::Unavailable
        ))
    ));
    committed_prepare
        .lose_ack
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let snapshot = committed_prepare.begin_read().unwrap();
    assert!(
        snapshot
            .get(Table::Mutations, prepare.mutation.as_bytes())
            .unwrap()
            .is_some()
    );
    assert_eq!(
        snapshot.scan_prefix(Table::Idempotency, &[]).unwrap().len(),
        1
    );
    drop(snapshot);
    assert!(matches!(
        prepare_fresh_arm(&committed_prepare, &prepare, &request),
        Err(FreshPreparationFailure::Existing)
    ));

    let fresh = prepare_fresh_arm(&store, &prepare, &request).unwrap();
    assert!(matches!(
        prepare_fresh_arm(&store, &prepare, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    *store.fail_next_commit.lock().unwrap() = true;
    assert!(matches!(
        fresh.arm(&arm, effect),
        Err(DurableArmFailure::CommitUnknown(_))
    ));
    assert!(matches!(
        prepare_fresh_arm(&store, &prepare, &request),
        Err(FreshPreparationFailure::Existing)
    ));
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, arm.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::Prepared);
    assert!(snapshot.outbox.is_empty());

    // The loss of the ACK after this fixation also does not allow the issuance of a second
    // ticket or the acquisition of a new right on the same idempotence.
    let stored = SwitchableStore {
        inner: MemoryStore::default(),
        lose_ack: std::sync::atomic::AtomicBool::new(false),
    };
    let mut tx = stored.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let fresh = prepare_fresh_arm(&stored, &prepare, &request).unwrap();
    stored
        .lose_ack
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        fresh.arm(&arm, effect),
        Err(DurableArmFailure::CommitUnknown(_))
    ));
    stored
        .lose_ack
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&stored, arm.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::DispatchArmed);
    assert!(matches!(
        prepare_fresh_arm(&stored, &prepare, &request),
        Err(FreshPreparationFailure::Existing)
    ));

    // Two independent performers receive the same request; only one can confirm new training
    // and receive a ticket.
    let concurrent = MemoryStore::default();
    let mut tx = concurrent.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let gate = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let run = || {
            gate.wait();
            match prepare_fresh_arm(&concurrent, &prepare, &request) {
                Ok(fresh) => fresh.arm(&arm, effect).is_ok(),
                Err(_) => false,
            }
        };
        let a = scope.spawn(run);
        let b = scope.spawn(run);
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|result| **result).count(), 1);
    assert!(matches!(
        prepare_fresh_arm(&concurrent, &prepare, &request),
        Err(FreshPreparationFailure::Existing)
    ));

    // After the process is closed, the right from Prepared is not restored: the journal
    // allows only a separate safe recovery.
    let path = std::env::temp_dir().join(format!(
        "carosign-fresh-arm-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let disk = crate::redb::backend::RedbStore::create_v3(&path).unwrap();
        let fresh = prepare_fresh_arm(&disk, &prepare, &request).unwrap();
        drop(fresh);
    }
    {
        let disk = crate::redb::backend::RedbStore::open_v3_guarded(&path).unwrap();
        assert!(matches!(
            prepare_fresh_arm(&disk, &prepare, &request),
            Err(FreshPreparationFailure::Existing)
        ));
        let snapshot =
            crate::recovery::read_mutation_snapshot_v3(&disk, arm.mutation, prepare.operation)
                .unwrap();
        assert_eq!(snapshot.mutation.phase, MutationPhase::Prepared);
        assert!(snapshot.outbox.is_empty());
    }
    std::fs::remove_file(path).unwrap();

    // Real redb serializes competing preparations in a single database; the result is also
    // verified after closing and reopening.
    let path = std::env::temp_dir().join(format!(
        "carosign-fresh-arm-race-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let disk = crate::redb::backend::RedbStore::create_v3(&path).unwrap();
        let gate = std::sync::Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let run = || {
                gate.wait();
                match prepare_fresh_arm(&disk, &prepare, &request) {
                    Ok(fresh) => fresh.arm(&arm, effect).is_ok(),
                    Err(_) => false,
                }
            };
            let a = scope.spawn(run);
            let b = scope.spawn(run);
            [a.join().unwrap(), b.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|result| **result).count(), 1);
    }
    {
        let disk = crate::redb::backend::RedbStore::open_v3_guarded(&path).unwrap();
        let snapshot =
            crate::recovery::read_mutation_snapshot_v3(&disk, arm.mutation, prepare.operation)
                .unwrap();
        assert_eq!(snapshot.mutation.phase, MutationPhase::DispatchArmed);
        assert!(matches!(
            prepare_fresh_arm(&disk, &prepare, &request),
            Err(FreshPreparationFailure::Existing)
        ));
    }
    std::fs::remove_file(path).unwrap();

    // Loss of response after present file commit is checked separately from MemoryStore:
    // After reopening, a second ticket cannot be issued.
    let path = std::env::temp_dir().join(format!(
        "carosign-fresh-arm-lost-ack-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let disk = crate::redb::backend::RedbStore::create_v3(&path).unwrap();
        let wrapped = LostAckFile {
            inner: &disk,
            lose_ack: std::sync::atomic::AtomicBool::new(false),
        };
        let fresh = prepare_fresh_arm(&wrapped, &prepare, &request).unwrap();
        wrapped
            .lose_ack
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            fresh.arm(&arm, effect),
            Err(DurableArmFailure::CommitUnknown(_))
        ));
    }
    {
        let disk = crate::redb::backend::RedbStore::open_v3_guarded(&path).unwrap();
        let snapshot =
            crate::recovery::read_mutation_snapshot_v3(&disk, arm.mutation, prepare.operation)
                .unwrap();
        assert_eq!(snapshot.mutation.phase, MutationPhase::DispatchArmed);
        assert!(matches!(
            prepare_fresh_arm(&disk, &prepare, &request),
            Err(FreshPreparationFailure::Existing)
        ));
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn lost_commit_ack_never_issues_a_second_ticket() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};

    use memory_lost_ack::LostAck;

    let sample = sample();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Meta,
        crate::redb::backend::SCHEMA_KEY,
        crate::redb::backend::SCHEMA_V3,
    )
    .unwrap();
    stage_prepared(&mut *tx, &prepare, &request).unwrap();
    tx.commit_durable().unwrap();
    let input = ArmMutation {
        mutation: prepare.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let expected_plan = prepare
        .plan
        .to_core(Scope {
            subject: prepare.subject,
            authenticator: prepare.authenticator,
        })
        .unwrap();
    let attempts = ArmAttemptOwner::new(1);
    assert!(matches!(
        attempts.arm(
            &LostAck(&store),
            prepare.operation,
            &input,
            effect,
            &expected_plan
        ),
        Err(ArmAttemptFailure::Durable(
            DurableArmFailure::CommitUnknown(StorageError::Unavailable)
        ))
    ));
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::DispatchArmed);
    assert!(!crate::recovery::may_replay(&snapshot.mutation));
    assert!(matches!(
        attempts.arm(&store, prepare.operation, &input, effect, &expected_plan),
        Err(ArmAttemptFailure::AlreadyAttempted)
    ));
}

#[test]
fn arm_stages_record_and_outbox_together_and_never_rearms_after_lost_ack() {
    use crate::store::{MetadataStore, tests_support::MemoryStore};
    use carosign_engine::{EffectSpec, PlannedEffect, TransitionIndex};
    let sample = sample();
    let prepare = PrepareMutation {
        mutation: sample.mutation,
        operation: sample.operation,
        subject: sample.subject,
        authenticator: sample.authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([13; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject: sample.subject,
                authenticator: sample.authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: prepare.operation,
        mutation: Some(prepare.mutation),
    };
    let store = MemoryStore::default();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_prepared(&mut *tx, &prepare, &request),
        Ok(Reservation::Staged)
    );
    tx.commit_durable().unwrap();
    let input = ArmMutation {
        mutation: prepare.mutation,
        worker: WorkerInstanceId::from_bytes([8; 16]).unwrap(),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([9; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(7), 2);
    let idem_key = crate::redb::codec::idempotency_key(&request.caller_digest, &request.key);
    let mut tx = store.begin_write().unwrap();
    let saved = tx.get(Table::Idempotency, &idem_key).unwrap().unwrap();
    tx.delete(Table::Idempotency, &idem_key).unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    assert!(enrollment_projection(&*tx, input.mutation, prepare.operation).is_err());
    assert!(stage_arm(&mut *tx, &input, effect).is_err());
    tx.abort();
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Idempotency, &idem_key, &saved).unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    let wrong = PlannedEffect::at(EffectSpec::SendVerify, effect.transition, effect.position);
    assert!(stage_arm(&mut *tx, &input, wrong).is_err());
    assert_eq!(stage_arm(&mut *tx, &input, effect), Ok(()));
    *store.fail_next_commit.lock().unwrap() = true;
    assert_eq!(tx.commit_durable(), Err(StorageError::Unavailable));
    let read = store.begin_read().unwrap();
    assert_eq!(
        decode_mutation(
            &read
                .get(Table::Mutations, input.mutation.as_bytes())
                .unwrap()
                .unwrap()
        )
        .unwrap()
        .phase,
        MutationPhase::Prepared
    );
    assert!(
        read.scan_prefix(Table::EffectOutbox, &[])
            .unwrap()
            .is_empty()
    );
    drop(read);
    let mut tx = store.begin_write().unwrap();
    stage_arm(&mut *tx, &input, effect).unwrap();
    // Imitate the lost confirmation after the actual fixation.
    let _lost_ack = tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    let record = decode_mutation(
        &read
            .get(Table::Mutations, input.mutation.as_bytes())
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record.phase, MutationPhase::DispatchArmed);
    assert!(!crate::recovery::may_replay(&record));
    let key = crate::redb::codec::outbox_key(record.operation.as_bytes(), 7, 2);
    let queued =
        decode_outbox(&key, &read.get(Table::EffectOutbox, &key).unwrap().unwrap()).unwrap();
    assert!(validate_mutation_outbox(&record, &queued).is_ok());
    drop(read);
    let mut retry = store.begin_write().unwrap();
    assert!(enrollment_projection(&*retry, prepare.mutation, prepare.operation).is_err());
    assert_eq!(
        stage_arm(&mut *retry, &input, effect),
        Err(StorageError::GenerationConflict)
    );
    retry.abort();

    let observation = ObservedMutation {
        mutation: input.mutation,
        operation: prepare.operation,
        command: input.command,
        worker: input.worker,
        epoch: input.epoch,
        effect: carosign_engine::transition::EffectKey {
            operation: prepare.operation,
            transition_index: effect.transition,
            position: carosign_engine::transition::EffectPosition::new(effect.position),
        },
        observation_digest: [11; 32],
        recovery_ref: Some(OpaqueProviderRef::new(vec![12]).unwrap()),
    };
    let mut tx = store.begin_write().unwrap();
    let mut foreign = observation.clone();
    foreign.worker = WorkerInstanceId::from_bytes([15; 16]).unwrap();
    assert!(stage_observation(&mut *tx, &foreign).is_err());
    stage_observation(&mut *tx, &observation).unwrap();
    *store.fail_next_commit.lock().unwrap() = true;
    assert_eq!(tx.commit_durable(), Err(StorageError::Unavailable));
    let read = store.begin_read().unwrap();
    let record = decode_mutation(
        &read
            .get(Table::Mutations, input.mutation.as_bytes())
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record.phase, MutationPhase::DispatchArmed);
    assert_eq!(
        decode_outbox(&key, &read.get(Table::EffectOutbox, &key).unwrap().unwrap())
            .unwrap()
            .phase,
        EffectPhase::Armed
    );
    drop(read);
    let mut tx = store.begin_write().unwrap();
    stage_observation(&mut *tx, &observation).unwrap();
    tx.commit_durable().unwrap();
    let read = store.begin_read().unwrap();
    let observed = decode_mutation(
        &read
            .get(Table::Mutations, input.mutation.as_bytes())
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(observed.phase, MutationPhase::ProviderObserved);
    assert_eq!(observed.provider_observation_digest, Some([11; 32]));
    assert_eq!(observed.recovery_ref, observation.recovery_ref);
    assert_eq!(
        decode_outbox(&key, &read.get(Table::EffectOutbox, &key).unwrap().unwrap())
            .unwrap()
            .phase,
        EffectPhase::Observed
    );
    assert_eq!(
        read.get(Table::MutationScopeLock, &observed.scope_key())
            .unwrap()
            .as_deref(),
        Some(input.mutation.as_bytes().as_slice())
    );
    drop(read);
    let scan = crate::recovery::StartupScan::scan_v3(&store).unwrap();
    assert_eq!(scan.needing_reconciliation, vec![input.mutation]);
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .unwrap();
    assert_eq!(snapshot.mutation.phase, MutationPhase::ProviderObserved);
    assert_eq!(
        snapshot.mutation.provider_observation_digest,
        Some([11; 32])
    );
    assert_eq!(snapshot.outbox.len(), 1);
    assert_eq!(snapshot.outbox[0].phase, EffectPhase::Observed);
    assert!(snapshot.enrollments.is_empty());
    // Loss of the replay record should not be converted into readiness or permit recovery
    // with incomplete atomic intent.
    let idem_key = crate::redb::codec::idempotency_key(&request.caller_digest, &request.key);
    let mut tx = store.begin_write().unwrap();
    let idem_value = tx.get(Table::Idempotency, &idem_key).unwrap().unwrap();
    tx.delete(Table::Idempotency, &idem_key).unwrap();
    tx.commit_durable().unwrap();
    assert!(crate::recovery::StartupScan::scan_v3(&store).is_err());
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Idempotency, &idem_key, &idem_value).unwrap();
    tx.commit_durable().unwrap();
    let recovered = crate::recovery::read_reconciliation_plan_v3(
        &store,
        input.mutation,
        prepare.operation,
        prepare.subject,
        prepare.authenticator,
        input.epoch,
    )
    .unwrap();
    assert_eq!(recovered.subject(), prepare.subject);
    assert_eq!(recovered.authenticator(), prepare.authenticator);
    assert_eq!(recovered.scope_digest(), &prepare.plan.scope_digest);
    assert!(
        crate::recovery::read_reconciliation_plan_v3(
            &store,
            input.mutation,
            prepare.operation,
            prepare.subject,
            prepare.authenticator,
            DeviceEpoch::new(input.epoch.value() + 1).unwrap(),
        )
        .is_err()
    );
    assert!(
        crate::recovery::read_reconciliation_plan_v3(
            &store,
            input.mutation,
            prepare.operation,
            prepare.subject,
            AuthenticatorId::from_bytes([77; 16]).unwrap(),
            input.epoch,
        )
        .is_err()
    );
    let orphan = crate::enrollment_codec::EnrollmentRecord {
        subject: snapshot.mutation.subject,
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([19; 16]).unwrap(),
            authenticator: snapshot.mutation.authenticator,
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: crate::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1]).unwrap(),
        format_id: "example".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        orphan.enrollment.id.as_bytes(),
        &crate::enrollment_codec::encode_v3(&orphan).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::Enrollments, orphan.enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    // NoChange It's fixed with the outcome of the queue and it's kept until it's finished,
    // and the loss of the queue doesn't have to look like a regular cleaning.
    let mut no_change = snapshot.mutation.clone();
    no_change.phase = MutationPhase::NoChange;
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        input.mutation.as_bytes(),
        &encode_mutation(&no_change).unwrap(),
    )
    .unwrap();
    tx.delete(Table::EffectOutbox, &key).unwrap();
    tx.commit_durable().unwrap();
    assert!(crate::recovery::StartupScan::scan_v3(&store).is_err());
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        input.mutation.as_bytes(),
        &encode_mutation(&snapshot.mutation).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::EffectOutbox,
        &key,
        &encode_outbox(&snapshot.outbox[0]).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_mutation_snapshot_v3(
            &store,
            input.mutation,
            OperationId::from_bytes([17; 16]).unwrap()
        )
        .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    let mut dangling = snapshot.mutation.scope_key().to_vec();
    dangling.extend_from_slice(&[18; 16]);
    tx.put(Table::EnrollmentIndex, &dangling, &[18; 16])
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.delete(Table::EnrollmentIndex, &dangling).unwrap();
    tx.delete(Table::MutationScopeLock, &snapshot.mutation.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(
        crate::recovery::read_mutation_snapshot_v3(&store, input.mutation, prepare.operation)
            .is_err()
    );
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::MutationScopeLock,
        &snapshot.mutation.scope_key(),
        input.mutation.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage_observation(&mut *tx, &observation),
        Err(StorageError::GenerationConflict)
    );
    tx.abort();
}

#[path = "mutation_test_fixtures/disk_lost_ack.rs"]
mod disk_lost_ack;
#[path = "mutation_test_fixtures/disk_lost_ack_tx.rs"]
mod disk_lost_ack_tx;
#[path = "mutation_test_fixtures/lost_ack_file.rs"]
mod lost_ack_file;
#[path = "mutation_test_fixtures/lost_arm_ack.rs"]
mod lost_arm_ack;
#[path = "mutation_test_fixtures/lost_arm_ack_transaction.rs"]
mod lost_arm_ack_transaction;
#[path = "mutation_test_fixtures/memory_lost_ack.rs"]
mod memory_lost_ack;
#[path = "mutation_test_fixtures/memory_lost_ack_tx.rs"]
mod memory_lost_ack_tx;
#[path = "mutation_test_fixtures/switchable_store.rs"]
mod switchable_store;
#[path = "mutation_test_fixtures/switchable_tx.rs"]
mod switchable_tx;
