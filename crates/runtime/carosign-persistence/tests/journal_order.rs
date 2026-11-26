//! Logic of the magazine on a shell vault: order, failures and cliff windows.
//!
//! ⚠️ It's the **order** and **failure** check, not the stability of the disc: the front
//! storage confirms the recording without proving power. The power failure test is a
//! stand-alone stand on the real back.

use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, EnrollmentId, MutationId, OperationId, SubjectId,
    WorkerInstanceId,
};
use carosign_persistence::journal::{
    ArmMutation, CommitEnrollment, MutationObservation, MutationRepository, PrepareMutation,
};
use carosign_persistence::journal_store::StoreJournal;
use carosign_persistence::model::{MutationClosure, MutationPhase, StorageError};
use carosign_persistence::redb::snapshot::JournalSnapshot;
use carosign_persistence::store::{MetadataStore, Table};
use carosign_persistence::test_support::MemoryStore;

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn mutation() -> MutationId {
    MutationId::from_bytes([4u8; 16]).expect("валид")
}

fn operation() -> OperationId {
    OperationId::from_bytes([5u8; 16]).expect("валид")
}

fn command() -> CommandId {
    CommandId::from_bytes([6u8; 16]).expect("валид")
}

fn worker() -> WorkerInstanceId {
    WorkerInstanceId::from_bytes([7u8; 16]).expect("валид")
}

fn enrollment() -> EnrollmentRef {
    EnrollmentRef {
        id: EnrollmentId::from_bytes([8u8; 16]).expect("валид"),
        authenticator: authenticator(),
        generation: EnrollmentGeneration::FIRST,
    }
}

fn prepare_input() -> PrepareMutation {
    PrepareMutation {
        mutation: mutation(),
        operation: operation(),
        subject: subject(),
        authenticator: authenticator(),
        expected_generation: None,
        enroll: true,
        plan: plan_for(subject(), authenticator()),
    }
}

#[test]
fn normal_journal_never_mutates_an_unmarked_schema() {
    let journal = StoreJournal::new(MemoryStore::default());
    assert!(journal.prepare(prepare_input()).is_err());
    assert!(journal.mark_uncertain(mutation()).is_err());
    let tx = journal.store().begin_read().unwrap();
    assert!(tx.scan_prefix(Table::Mutations, &[]).unwrap().is_empty());
    assert!(
        tx.scan_prefix(Table::MutationScopeLock, &[])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn fixture_mode_never_overrides_a_present_unknown_marker() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    let mut tx = journal.store().begin_write().unwrap();
    tx.put(Table::Meta, b"biometric.schema", b"unknown")
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(journal.prepare(prepare_input()).is_err());
    assert!(journal.mark_uncertain(mutation()).is_err());
    let tx = journal.store().begin_read().unwrap();
    assert_eq!(
        tx.get(Table::Meta, b"biometric.schema").unwrap().as_deref(),
        Some(&b"unknown"[..])
    );
    assert!(tx.scan_prefix(Table::Mutations, &[]).unwrap().is_empty());
}

fn arm_input() -> ArmMutation {
    ArmMutation {
        mutation: mutation(),
        worker: worker(),
        epoch: DeviceEpoch::FIRST,
        command: command(),
        recovery_ref: None,
    }
}

fn prepared() -> StoreJournal<MemoryStore> {
    let store = MemoryStore::default();
    let journal = StoreJournal::legacy_fixture(store);
    journal.prepare(prepare_input()).expect("готовится");
    journal
}

/// Plan for inspections: new registration without previous images.
fn plan_for(
    subject: SubjectRef,
    authenticator: AuthenticatorId,
) -> carosign_persistence::mutation_plan::MutationPlan {
    carosign_persistence::mutation_plan::MutationPlan::build(
        carosign_persistence::mutation_plan::Transition::EnrollNew,
        None,
        Some(carosign_core::ids::EnrollmentId::from_bytes([8u8; 16]).expect("валид")),
        1,
        Vec::new(),
        carosign_persistence::mutation_plan::Scope {
            subject,
            authenticator,
        },
    )
    .expect("план")
}

#[test]
fn prepare_locks_scope_before_any_hardware_work() {
    let journal = prepared();
    let record = journal.record(mutation()).expect("запись есть");
    assert_eq!(record.phase, MutationPhase::Prepared);
    // Intent is recorded, but sending is still prohibited.
    assert!(!record.allows_dispatch());
    assert_eq!(
        journal.scope_lock(&record.scope_key()).expect("читается"),
        Some(mutation())
    );
}

#[test]
fn second_prepare_on_same_scope_is_refused() {
    let journal = prepared();
    let second = journal.prepare(PrepareMutation {
        mutation: MutationId::from_bytes([9u8; 16]).expect("валид"),
        ..prepare_input()
    });
    assert_eq!(second.unwrap_err(), StorageError::GenerationConflict);
}

#[test]
fn duplicate_mutation_id_in_another_scope_never_overwrites_journal() {
    let journal = prepared();
    let before = journal.record(mutation()).unwrap();
    let other_subject = SubjectRef {
        authority: subject().authority,
        subject: SubjectId::from_bytes([42; 16]).unwrap(),
    };
    let input = PrepareMutation {
        subject: other_subject,
        plan: plan_for(other_subject, authenticator()),
        ..prepare_input()
    };
    assert_eq!(
        journal.prepare(input).unwrap_err(),
        StorageError::GenerationConflict
    );
    assert_eq!(journal.record(mutation()).unwrap(), before);
    assert_eq!(
        journal.scope_lock(&before.scope_key()).unwrap(),
        Some(mutation())
    );
}

#[test]
fn prepare_rejects_plan_for_another_scope_or_changed_digest() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    let mut input = prepare_input();
    input.subject = SubjectRef {
        authority: subject().authority,
        subject: SubjectId::from_bytes([42; 16]).unwrap(),
    };
    assert!(matches!(
        journal.prepare(input),
        Err(StorageError::Corrupt { .. })
    ));
    let mut input = prepare_input();
    input.plan.scope_digest[0] ^= 1;
    assert!(matches!(
        journal.prepare(input),
        Err(StorageError::Corrupt { .. })
    ));
    assert_eq!(count_rows(&journal, Table::Mutations), 0);
    assert_eq!(count_rows(&journal, Table::MutationScopeLock), 0);
}

#[test]
fn shared_prepare_validator_rejects_inconsistent_requests() {
    let valid = prepare_input();
    assert!(carosign_persistence::journal::validate_prepare(&valid).is_ok());
    let mut wrong_kind = valid.clone();
    wrong_kind.enroll = false;
    assert!(carosign_persistence::journal::validate_prepare(&wrong_kind).is_err());
    let mut tampered = valid;
    tampered.plan.scope_digest[0] ^= 1;
    assert!(carosign_persistence::journal::validate_prepare(&tampered).is_err());
}

#[test]
fn journal_decoder_reads_only_validated_existing_records() {
    let journal = prepared();
    let record = journal.record(mutation()).unwrap();
    let bytes = carosign_persistence::redb::record::encode_mutation(&record).unwrap();
    assert_eq!(
        carosign_persistence::journal::decode_record(&bytes).unwrap(),
        record
    );
    let mut corrupted = bytes;
    corrupted[12] ^= 1;
    assert!(carosign_persistence::journal::decode_record(&corrupted).is_err());
}

#[test]
fn arm_requires_matching_scope_lock_and_never_overwrites_outbox() {
    for foreign in [None, Some(MutationId::from_bytes([9; 16]).unwrap())] {
        let journal = prepared();
        let record = journal.record(mutation()).unwrap();
        let mut tx = journal.store().begin_write().unwrap();
        match foreign {
            Some(id) => tx
                .put(Table::MutationScopeLock, &record.scope_key(), id.as_bytes())
                .unwrap(),
            None => tx
                .delete(Table::MutationScopeLock, &record.scope_key())
                .unwrap(),
        }
        tx.commit_durable().unwrap();
        assert!(matches!(
            journal.arm(arm_input()),
            Err(StorageError::Corrupt { .. })
        ));
        assert_eq!(
            journal.record(mutation()).unwrap().phase,
            MutationPhase::Prepared
        );
        assert_eq!(count_rows(&journal, Table::EffectOutbox), 0);
    }
    let journal = prepared();
    let key = carosign_persistence::redb::codec::outbox_key(operation().as_bytes(), 0, 0);
    let mut tx = journal.store().begin_write().unwrap();
    tx.put(Table::EffectOutbox, &key, b"existing").unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        journal.arm(arm_input()),
        Err(StorageError::GenerationConflict)
    ));
    assert_eq!(
        journal.record(mutation()).unwrap().phase,
        MutationPhase::Prepared
    );
    assert_eq!(
        journal
            .store()
            .begin_read()
            .unwrap()
            .get(Table::EffectOutbox, &key)
            .unwrap()
            .as_deref(),
        Some(&b"existing"[..])
    );
}

#[test]
fn arm_is_refused_when_durable_write_fails() {
    // D-03: **up to** permission breaks - no repeat, no dispatch allowed.
    let journal = prepared();
    *journal_store_flag(&journal).lock().expect("замок") = true;

    let armed = journal.arm(arm_input());
    assert_eq!(armed.unwrap_err(), StorageError::Unavailable);

    let record = journal.record(mutation()).expect("запись есть");
    assert_eq!(record.phase, MutationPhase::Prepared);
    assert!(!record.allows_dispatch());
    let key = carosign_persistence::redb::codec::outbox_key(record.operation.as_bytes(), 0, 0);
    let tx = journal.store().begin_read().unwrap();
    assert!(
        tx.get(carosign_persistence::store::Table::EffectOutbox, &key)
            .unwrap()
            .is_none()
    );
    drop(tx);
    let scan = carosign_persistence::recovery::StartupScan::scan(journal.store()).unwrap();
    assert_eq!(scan.safe_to_close, vec![mutation()]);
    assert!(scan.needing_reconciliation.is_empty());
}

#[test]
fn lost_arm_commit_ack_never_issues_ticket_or_replays() {
    use std::sync::atomic::Ordering;

    use lost_ack_store::LostAckStore;

    let journal = StoreJournal::legacy_fixture(LostAckStore::default());
    journal.prepare(prepare_input()).unwrap();
    journal.store().lose_ack.store(true, Ordering::SeqCst);
    assert!(matches!(
        journal.arm(arm_input()),
        Err(StorageError::Unavailable)
    ));
    let record = journal.record(mutation()).unwrap();
    assert_eq!(record.phase, MutationPhase::DispatchArmed);
    assert!(!carosign_persistence::recovery::may_replay(&record));
    assert!(journal.arm(arm_input()).is_err());
    let scan = carosign_persistence::recovery::StartupScan::scan(journal.store()).unwrap();
    assert_eq!(scan.needing_reconciliation, vec![mutation()]);
}

#[test]
fn arm_requires_prepared_state() {
    let store = MemoryStore::default();
    let journal = StoreJournal::legacy_fixture(store);
    // No. `prepare` There's nothing to write down.
    assert!(journal.arm(arm_input()).is_err());
}

fn insert_registration(store: &dyn MetadataStore) {
    use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    let record = EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([11; 16]).unwrap(),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: String::new(),
        status: EnrollmentStatus::Active,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![1]).unwrap(),
        format_id: "format".into(),
        format_version: 1,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        record.enrollment.id.as_bytes(),
        &carosign_persistence::enrollment_codec::encode(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::EnrollmentIndex,
        &carosign_persistence::redb::codec::enrollment_index_key(&record),
        record.enrollment.id.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
}

#[test]
fn prepare_refuses_plan_missing_an_existing_binding() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    insert_registration(journal.store());
    assert_eq!(
        journal.prepare(prepare_input()).unwrap_err(),
        StorageError::GenerationConflict
    );
    assert_eq!(count_rows(&journal, Table::Mutations), 0);
    assert_eq!(count_rows(&journal, Table::MutationScopeLock), 0);
}

#[test]
fn arm_refuses_scope_changed_after_prepare_and_retains_lock() {
    let journal = prepared();
    insert_registration(journal.store());
    assert_eq!(
        journal.arm(arm_input()).unwrap_err(),
        StorageError::GenerationConflict
    );
    let record = journal.record(mutation()).unwrap();
    assert_eq!(record.phase, MutationPhase::Prepared);
    assert_eq!(
        journal.scope_lock(&record.scope_key()).unwrap(),
        Some(mutation())
    );
    assert_eq!(count_rows(&journal, Table::EffectOutbox), 0);
}

#[test]
fn commit_registry_requires_observation() {
    let journal = prepared();
    let _ticket = journal.arm(arm_input()).expect("разрешено");
    // You can't fix it without surveillance.
    let result = journal.commit_registry(CommitEnrollment {
        mutation: mutation(),
        enrollment: enrollment(),
        enroll: true,
    });
    assert_eq!(result.unwrap_err(), StorageError::GenerationConflict);
}

#[test]
fn uncertain_keeps_scope_locked() {
    // D-04: once the break is resolved, check, block **not ** removed.
    let journal = prepared();
    let _ticket = journal.arm(arm_input()).expect("разрешено");
    journal.mark_uncertain(mutation()).expect("помечено");

    let record = journal.record(mutation()).expect("запись есть");
    assert_eq!(record.phase, MutationPhase::NeedsReconciliation);
    assert_eq!(
        journal.scope_lock(&record.scope_key()).expect("читается"),
        Some(mutation())
    );
    assert_eq!(
        journal.finalize(mutation(), MutationClosure::NeedsReconciliation),
        Ok(()) // turn into mark_uncertainthe area remains
    );
    assert_eq!(
        journal.scope_lock(&record.scope_key()).expect("читается"),
        Some(mutation())
    );
}

#[test]
fn armed_mutation_cannot_be_closed_without_proof() {
    let journal = prepared();
    let _ticket = journal.arm(arm_input()).expect("разрешено");
    assert!(
        journal
            .record_observation(MutationObservation {
                mutation: mutation(),
                phase: MutationPhase::NoChange,
                recovery_ref: None,
            })
            .is_err()
    );
    assert!(
        journal
            .finalize(mutation(), MutationClosure::NoChange)
            .is_err()
    );
    assert!(
        journal
            .finalize(mutation(), MutationClosure::Completed)
            .is_err()
    );

    let record = journal.record(mutation()).expect("запись есть");
    assert_eq!(record.phase, MutationPhase::DispatchArmed);
    assert_eq!(
        journal.scope_lock(&record.scope_key()).expect("читается"),
        Some(mutation())
    );
}

#[test]
fn scan_incomplete_lists_only_open_mutations() {
    let journal = prepared();
    let records = journal.scan_incomplete().expect("читается");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].phase, MutationPhase::Prepared);

    let snapshot = JournalSnapshot::from_records(records);
    assert_eq!(snapshot.incomplete().len(), 1);
    // `Prepared` I haven't touched the hardware yet.
    assert!(snapshot.may_have_touched_hardware().is_empty());
}

#[test]
fn snapshot_knows_armed_mutations_touch_hardware() {
    let journal = prepared();
    let _ticket = journal.arm(arm_input()).expect("разрешено");
    let snapshot = JournalSnapshot::from_records(journal.scan_incomplete().expect("читается"));
    assert_eq!(snapshot.may_have_touched_hardware().len(), 1);
}

#[test]
fn outbox_record_exists_only_after_arm() {
    // ⚠️ The queue entry appears **equally** along with the permission.
    let journal = prepared();
    let before = count_rows(&journal, Table::EffectOutbox);
    assert_eq!(before, 0);
    let _ticket = journal.arm(arm_input()).expect("разрешено");
    assert_eq!(count_rows(&journal, Table::EffectOutbox), 1);
}

fn count_rows(journal: &StoreJournal<MemoryStore>, table: Table) -> usize {
    journal
        .store()
        .begin_read()
        .expect("чтение")
        .scan_prefix(table, &[])
        .expect("сканирование")
        .len()
}

fn journal_store_flag(journal: &StoreJournal<MemoryStore>) -> &std::sync::Mutex<bool> {
    &journal.store().fail_next_commit
}

#[path = "journal_order_fixtures/lost_ack_store.rs"]
mod lost_ack_store;
#[path = "journal_order_fixtures/lost_ack_write.rs"]
mod lost_ack_write;
