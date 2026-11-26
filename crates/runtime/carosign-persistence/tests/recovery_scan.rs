//! Recovery: Scanning the log and preventing repetition.
//!
//! On startup the service identifies fenced scopes and never resends a mutation command in
//! any phase. Power-loss testing requires a separate fixture.

use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, MutationId, OperationId, SubjectId, WorkerInstanceId,
};
use carosign_persistence::journal::{
    ArmMutation, MutationObservation, MutationRepository, PrepareMutation,
};
use carosign_persistence::journal_store::StoreJournal;
use carosign_persistence::model::{MutationClosure, MutationPhase};
use carosign_persistence::recovery::{RecoveryDecision, StartupScan, decide_for, may_replay};
use carosign_persistence::store::MetadataStore;
use carosign_persistence::test_support::MemoryStore;

fn prepare(id: u8) -> PrepareMutation {
    PrepareMutation {
        mutation: MutationId::from_bytes([id; 16]).expect("валид"),
        operation: OperationId::from_bytes([id + 100; 16]).expect("валид"),
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
        },
        authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
        expected_generation: Some(EnrollmentGeneration::FIRST),
        enroll: true,
        plan: plan_for(
            SubjectRef {
                authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
                subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
            },
            AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
        ),
    }
}

fn arm(id: u8) -> ArmMutation {
    ArmMutation {
        mutation: MutationId::from_bytes([id; 16]).expect("валид"),
        worker: WorkerInstanceId::from_bytes([9u8; 16]).expect("валид"),
        epoch: DeviceEpoch::FIRST,
        command: CommandId::from_bytes([8u8; 16]).expect("валид"),
        recovery_ref: None,
    }
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
fn prepared_mutation_is_safe_to_close() {
    // D-02: cliff after intent, until permission - nothing sent.
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(1)).expect("готовится");

    let scan = StartupScan::scan(journal.store()).expect("сканируется");
    assert_eq!(scan.safe_to_close.len(), 1);
    assert!(scan.needing_reconciliation.is_empty());
    assert!(scan.allows_ready());
}

#[test]
fn direct_mutation_read_rejects_record_under_foreign_key() {
    use carosign_persistence::store::Table;
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(1)).unwrap();
    let original = journal
        .record(MutationId::from_bytes([1; 16]).unwrap())
        .unwrap();
    let foreign = MutationId::from_bytes([2; 16]).unwrap();
    let mut tx = journal.store().begin_write().unwrap();
    tx.put(
        Table::Mutations,
        foreign.as_bytes(),
        &carosign_persistence::redb::record::encode_mutation(&original).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(journal.record(foreign).is_err());
    assert!(journal.scan_incomplete().is_err());
    assert!(journal.mark_uncertain(foreign).is_err());
}

#[test]
fn journal_updates_require_the_scope_lock() {
    use carosign_persistence::store::Table;
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(1)).unwrap();
    journal.arm(arm(1)).unwrap();
    let id = MutationId::from_bytes([1; 16]).unwrap();
    let before = journal.record(id).unwrap();
    let mut tx = journal.store().begin_write().unwrap();
    tx.delete(Table::MutationScopeLock, &before.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(journal.mark_uncertain(id).is_err());
    assert!(
        journal
            .record_observation(MutationObservation {
                mutation: id,
                phase: MutationPhase::ProviderObserved,
                recovery_ref: None,
            })
            .is_err()
    );
    assert_eq!(journal.record(id).unwrap(), before);
}

#[test]
fn journal_does_not_advance_armed_mutation_with_missing_or_foreign_outbox() {
    use carosign_persistence::store::Table;
    for replace in [false, true] {
        let journal = StoreJournal::legacy_fixture(MemoryStore::default());
        journal.prepare(prepare(1)).unwrap();
        journal.arm(arm(1)).unwrap();
        let id = MutationId::from_bytes([1; 16]).unwrap();
        let before = journal.record(id).unwrap();
        let key = carosign_persistence::redb::codec::outbox_key(before.operation.as_bytes(), 0, 0);
        let mut tx = journal.store().begin_write().unwrap();
        if replace {
            tx.put(Table::EffectOutbox, &key, &[4; 16]).unwrap();
        } else {
            tx.delete(Table::EffectOutbox, &key).unwrap();
        }
        tx.commit_durable().unwrap();
        assert!(
            journal
                .record_observation(MutationObservation {
                    mutation: id,
                    phase: MutationPhase::ProviderObserved,
                    recovery_ref: None,
                })
                .is_err()
        );
        assert!(journal.mark_uncertain(id).is_err());
        assert_eq!(journal.record(id).unwrap(), before);
    }
}

#[test]
fn uncertain_marker_preserves_possible_committed_metadata_after_lost_ack() {
    use carosign_persistence::store::Table;
    for phase in [
        MutationPhase::RegistryCommitted,
        MutationPhase::NoChange,
        MutationPhase::Completed,
    ] {
        let journal = StoreJournal::legacy_fixture(MemoryStore::default());
        journal.prepare(prepare(1)).unwrap();
        journal.arm(arm(1)).unwrap();
        let id = MutationId::from_bytes([1; 16]).unwrap();
        let mut record = journal.record(id).unwrap();
        record.phase = phase;
        let mut tx = journal.store().begin_write().unwrap();
        let encoded = carosign_persistence::redb::record::encode_mutation(&record).unwrap();
        tx.put(Table::Mutations, id.as_bytes(), &encoded).unwrap();
        tx.commit_durable().unwrap();
        assert!(journal.mark_uncertain(id).is_err());
        let tx = journal.store().begin_read().unwrap();
        assert_eq!(
            tx.get(Table::Mutations, id.as_bytes()).unwrap().unwrap(),
            encoded
        );
        assert_eq!(
            tx.get(Table::MutationScopeLock, &record.scope_key())
                .unwrap()
                .unwrap(),
            id.as_bytes()
        );
    }
}

#[test]
fn startup_keeps_scope_blocked_after_lost_commit_ack() {
    use carosign_persistence::store::Table;
    for phase in [MutationPhase::RegistryCommitted, MutationPhase::NoChange] {
        let journal = StoreJournal::legacy_fixture(MemoryStore::default());
        journal.prepare(prepare(1)).unwrap();
        journal.arm(arm(1)).unwrap();
        let id = MutationId::from_bytes([1; 16]).unwrap();
        let mut record = journal.record(id).unwrap();
        record.phase = phase;
        let encoded = carosign_persistence::redb::record::encode_mutation(&record).unwrap();
        let mut tx = journal.store().begin_write().unwrap();
        tx.put(Table::Mutations, id.as_bytes(), &encoded).unwrap();
        tx.commit_durable().unwrap();

        assert!(journal.mark_uncertain(id).is_err());
        let scan = StartupScan::scan(journal.store()).unwrap();
        assert!(scan.needing_reconciliation.contains(&id) || scan.needs_decision.contains(&id));
        assert!(
            carosign_persistence::recovery::scopes_to_block(journal.store())
                .unwrap()
                .contains(&id)
        );
        let read = journal.store().begin_read().unwrap();
        assert_eq!(
            read.get(Table::Mutations, id.as_bytes()).unwrap(),
            Some(encoded)
        );
        assert_eq!(
            read.get(Table::MutationScopeLock, &record.scope_key())
                .unwrap()
                .as_deref(),
            Some(id.as_bytes().as_slice())
        );
    }
}

#[test]
fn armed_mutation_blocks_and_never_replays() {
    // D-03/D-04: Once authorized, lock and check, without repeat.
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(2)).expect("готовится");
    let _ticket = journal.arm(arm(2)).expect("разрешено");

    let scan = StartupScan::scan(journal.store()).expect("сканируется");
    assert_eq!(scan.needing_reconciliation.len(), 1);
    assert!(scan.safe_to_close.is_empty());

    let record = journal
        .record(MutationId::from_bytes([2u8; 16]).expect("валид"))
        .expect("читается");
    assert!(!may_replay(&record));
    assert_eq!(decide_for(&record), RecoveryDecision::BlockAndReconcile);
}

#[test]
fn uncertain_armed_mutation_requires_its_outbox_record() {
    use carosign_persistence::store::Table;
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(2)).unwrap();
    journal.arm(arm(2)).unwrap();
    let id = MutationId::from_bytes([2; 16]).unwrap();
    journal.mark_uncertain(id).unwrap();
    assert_eq!(
        StartupScan::scan(journal.store())
            .unwrap()
            .needing_reconciliation,
        vec![id]
    );

    let record = journal.record(id).unwrap();
    let key = carosign_persistence::redb::codec::outbox_key(record.operation.as_bytes(), 0, 0);
    let mut tx = journal.store().begin_write().unwrap();
    tx.delete(Table::EffectOutbox, &key).unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan(journal.store()).is_err());
}

#[test]
fn observed_but_uncommitted_still_reconciles() {
    // D-05: ISP responded, but no metadate attached.
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(3)).expect("готовится");
    let _ticket = journal.arm(arm(3)).expect("разрешено");
    journal
        .record_observation(MutationObservation {
            mutation: MutationId::from_bytes([3u8; 16]).expect("валид"),
            phase: MutationPhase::ProviderObserved,
            recovery_ref: None,
        })
        .expect("наблюдение");

    let scan = StartupScan::scan(journal.store()).expect("сканируется");
    assert_eq!(scan.needing_reconciliation.len(), 1);
}

#[test]
fn prepared_mutation_is_not_falsely_closed() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(4)).expect("готовится");
    assert!(
        journal
            .finalize(
                MutationId::from_bytes([4u8; 16]).expect("валид"),
                MutationClosure::NoChange,
            )
            .is_err()
    );

    let scan = StartupScan::scan(journal.store()).expect("сканируется");
    assert_eq!(scan.safe_to_close.len(), 1);
    assert!(scan.needing_reconciliation.is_empty());
    assert!(scan.needs_decision.is_empty());
    assert!(scan.closed.is_empty());
    assert_eq!(
        carosign_persistence::recovery::scopes_to_block(journal.store()).unwrap(),
        vec![MutationId::from_bytes([4; 16]).unwrap()]
    );
}

#[test]
fn historical_terminal_phase_without_outcome_needs_decision() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(4)).unwrap();
    let mut record = journal
        .record(MutationId::from_bytes([4; 16]).unwrap())
        .unwrap();
    for phase in [MutationPhase::NoChange, MutationPhase::Completed] {
        record.phase = phase;
        assert_eq!(decide_for(&record), RecoveryDecision::NeedsDecision);
    }
}

#[test]
fn prepared_without_valid_plan_is_not_safe_to_close() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(4)).unwrap();
    let mut record = journal
        .record(MutationId::from_bytes([4; 16]).unwrap())
        .unwrap();
    let mut corrupt = record.clone();
    corrupt.plan.as_mut().unwrap().scope_digest[0] ^= 1;
    assert_eq!(decide_for(&corrupt), RecoveryDecision::NeedsDecision);
    record.plan = None;
    assert_eq!(decide_for(&record), RecoveryDecision::NeedsDecision);
}

#[test]
fn prepared_binding_is_allowed_but_armed_binding_must_be_complete() {
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(4)).unwrap();
    let id = MutationId::from_bytes([4; 16]).unwrap();
    let original = journal.record(id).unwrap();
    let mut inconsistent = original.clone();
    inconsistent.worker = Some(WorkerInstanceId::from_bytes([8; 16]).unwrap());
    assert_eq!(decide_for(&inconsistent), RecoveryDecision::CloseAsNoChange);
    inconsistent = original;
    inconsistent.phase = MutationPhase::DispatchArmed;
    assert_eq!(decide_for(&inconsistent), RecoveryDecision::NeedsDecision);
}

#[test]
fn startup_rejects_missing_or_foreign_outbox_entries() {
    use carosign_persistence::store::Table;
    for corruption in 0..3 {
        let journal = StoreJournal::legacy_fixture(MemoryStore::default());
        journal.prepare(prepare(4)).unwrap();
        journal.arm(arm(4)).unwrap();
        let record = journal
            .record(MutationId::from_bytes([4; 16]).unwrap())
            .unwrap();
        let key = carosign_persistence::redb::codec::outbox_key(record.operation.as_bytes(), 0, 0);
        let mut tx = journal.store().begin_write().unwrap();
        match corruption {
            0 => tx.delete(Table::EffectOutbox, &key).unwrap(),
            1 => tx.put(Table::EffectOutbox, &key, &[9; 16]).unwrap(),
            _ => tx.put(Table::EffectOutbox, &[8; 28], &[9; 16]).unwrap(),
        }
        tx.commit_durable().unwrap();
        assert!(StartupScan::scan(journal.store()).is_err());
    }
}

#[test]
fn unverified_completed_record_without_lock_fails_closed() {
    use carosign_persistence::store::Table;
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(4)).unwrap();
    let id = MutationId::from_bytes([4; 16]).unwrap();
    let mut record = journal.record(id).unwrap();
    record.phase = MutationPhase::Completed;
    let mut tx = journal.store().begin_write().unwrap();
    tx.put(
        Table::Mutations,
        id.as_bytes(),
        &carosign_persistence::redb::record::encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.delete(Table::MutationScopeLock, &record.scope_key())
        .unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan(journal.store()).is_err());
}

#[test]
fn legacy_outcomes_remain_in_incomplete_and_blocked_lists() {
    use carosign_persistence::redb::snapshot::JournalSnapshot;
    use carosign_persistence::store::Table;
    for phase in [MutationPhase::NoChange, MutationPhase::Completed] {
        let journal = StoreJournal::legacy_fixture(MemoryStore::default());
        journal.prepare(prepare(4)).unwrap();
        let id = MutationId::from_bytes([4; 16]).unwrap();
        let mut record = journal.record(id).unwrap();
        record.phase = phase;
        let mut tx = journal.store().begin_write().unwrap();
        tx.put(
            Table::Mutations,
            id.as_bytes(),
            &carosign_persistence::redb::record::encode_mutation(&record).unwrap(),
        )
        .unwrap();
        tx.commit_durable().unwrap();
        assert_eq!(
            StartupScan::scan(journal.store()).unwrap().needs_decision,
            vec![id]
        );
        assert_eq!(
            carosign_persistence::recovery::scopes_to_block(journal.store()).unwrap(),
            vec![id]
        );
        let incomplete = journal.scan_incomplete().unwrap();
        assert_eq!(incomplete.len(), 1);
        assert_eq!(
            JournalSnapshot::from_records(incomplete).incomplete().len(),
            1
        );
    }
    assert!(!MutationPhase::NoChange.is_terminal());
    assert!(MutationPhase::NoChange.may_have_touched_hardware());
}

#[test]
fn garbage_in_journal_fails_closed() {
    // D-07: A corrupted record is not "repaired" or missed.
    let store = MemoryStore::default();
    {
        use carosign_persistence::store::Table;
        let mut tx = store.begin_write().expect("запись");
        tx.put(
            Table::Mutations,
            MutationId::from_bytes([5u8; 16]).expect("валид").as_bytes(),
            b"not an envelope",
        )
        .expect("положилось");
        tx.commit_durable().expect("зафиксировано");
    }

    assert!(StartupScan::scan(&store).is_err());
}

#[test]
fn recovery_rejects_missing_or_foreign_scope_lock() {
    use carosign_persistence::store::Table;
    for foreign in [None, Some(MutationId::from_bytes([9; 16]).unwrap())] {
        let journal = StoreJournal::legacy_fixture(MemoryStore::default());
        journal.prepare(prepare(2)).unwrap();
        journal.arm(arm(2)).unwrap();
        let id = MutationId::from_bytes([2; 16]).unwrap();
        let scope = journal.record(id).unwrap().scope_key();
        let mut tx = journal.store().begin_write().unwrap();
        match foreign {
            None => tx.delete(Table::MutationScopeLock, &scope).unwrap(),
            Some(other) => tx
                .put(Table::MutationScopeLock, &scope, other.as_bytes())
                .unwrap(),
        }
        tx.commit_durable().unwrap();
        assert!(StartupScan::scan(journal.store()).is_err());
        assert!(carosign_persistence::recovery::scopes_to_block(journal.store()).is_err());
        assert_eq!(
            journal.record(id).unwrap().phase,
            MutationPhase::DispatchArmed
        );
    }
}

#[test]
fn recovery_rejects_orphan_lock_and_mismatched_mutation_key() {
    use carosign_persistence::store::Table;
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    journal.prepare(prepare(2)).unwrap();
    let id = MutationId::from_bytes([2; 16]).unwrap();
    let mut tx = journal.store().begin_write().unwrap();
    let encoded = tx.get(Table::Mutations, id.as_bytes()).unwrap().unwrap();
    tx.delete(Table::Mutations, id.as_bytes()).unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan(journal.store()).is_err());

    let mut tx = journal.store().begin_write().unwrap();
    tx.put(Table::Mutations, &[9; 16], &encoded).unwrap();
    tx.commit_durable().unwrap();
    assert!(StartupScan::scan(journal.store()).is_err());
}
