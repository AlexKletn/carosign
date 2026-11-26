//! The Registry reads and writes the actual records. `carosign.storage.v1` (document 23, 33).
//!
//! ⚠️ Checking out what used to be a stub: `by_id` return `Corrupt` on any record, and
//! `by_scope` Now the record is collected, put down, read and indexed by the same**
//! transaction.

use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, EnrollmentId, MutationId, OperationId, SubjectId,
};
use carosign_persistence::enrollment_codec::{self, EnrollmentRecord, EnrollmentStatus};
use carosign_persistence::journal::{MutationObservation, MutationRepository, PrepareMutation};
use carosign_persistence::journal_store::StoreJournal;
use carosign_persistence::model::MutationPhase;
use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::registry::{Lookup, RegistryReader};
use carosign_persistence::store::MetadataStore;
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("registry-{name}-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn record(byte: u8) -> EnrollmentRecord {
    EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
            authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![byte; 4]).expect("годится"),
        format_id: "goodix-asp".into(),
        format_version: 1,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    }
}

/// Plan for inspections: new registration without previous images.
fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

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
fn record_round_trips_through_the_registry() {
    let path = temp_path("roundtrip");
    let store = RedbStore::create(&path).expect("создано");
    let original = record(7);
    {
        let mut tx = store.begin_write().expect("запись");
        let bytes = enrollment_codec::encode(&original).expect("кодируется");
        tx.put(
            carosign_persistence::store::Table::Enrollments,
            original.enrollment.id.as_bytes(),
            &bytes,
        )
        .expect("положено");
        tx.commit_durable().expect("зафиксировано");
    }
    let reader = RegistryReader::new(&store);
    assert_eq!(
        reader.by_id(original.enrollment.id).expect("читается"),
        Lookup::Found(original)
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn journal_enrollment_read_rejects_record_under_foreign_key() {
    use carosign_persistence::store::Table;
    use carosign_persistence::test_support::MemoryStore;
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    let foreign = EnrollmentId::from_bytes([8; 16]).unwrap();
    let mut tx = journal.store().begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        foreign.as_bytes(),
        &enrollment_codec::encode(&record(7)).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(journal.enrollment(foreign).is_err());
}

#[test]
fn unproven_commit_never_changes_registry_or_scope_lock() {
    let path = temp_path("commit");
    let store = RedbStore::create(&path).expect("создано");
    let journal = StoreJournal::legacy_fixture(store);
    let mutation = MutationId::from_bytes([4u8; 16]).expect("валид");
    journal
        .prepare(PrepareMutation {
            mutation,
            operation: OperationId::from_bytes([5u8; 16]).expect("валид"),
            subject: subject(),
            authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
            expected_generation: None,
            enroll: true,
            plan: plan_for(subject(), authenticator()),
        })
        .expect("готовится");
    journal
        .arm(carosign_persistence::journal::ArmMutation {
            mutation,
            worker: carosign_core::ids::WorkerInstanceId::from_bytes([7; 16]).unwrap(),
            epoch: carosign_core::device::DeviceEpoch::FIRST,
            command: carosign_core::ids::CommandId::from_bytes([6; 16]).unwrap(),
            recovery_ref: None,
        })
        .unwrap();
    journal
        .record_observation(MutationObservation {
            mutation,
            phase: MutationPhase::ProviderObserved,
            recovery_ref: None,
        })
        .expect("наблюдение");
    let result = journal.commit_registry(carosign_persistence::journal::CommitEnrollment {
        mutation,
        enrollment: record(8).enrollment,
        enroll: true,
    });
    assert!(matches!(
        result,
        Err(carosign_persistence::model::StorageError::Unavailable)
    ));

    let reader = RegistryReader::new(journal.store());
    assert!(matches!(
        reader.by_id(record(8).enrollment.id).expect("читается"),
        Lookup::Missing
    ));
    let mut scope = [0u8; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope[..carosign_persistence::limits::SUBJECT_CANONICAL_LEN]
        .copy_from_slice(&subject().to_canonical());
    scope[carosign_persistence::limits::SUBJECT_CANONICAL_LEN..].copy_from_slice(&[3u8; 16]);
    let found = reader.by_scope(&scope).expect("по области");
    assert!(found.is_empty());
    assert_eq!(journal.scope_lock(&scope).unwrap(), Some(mutation));
    assert_eq!(
        journal.record(mutation).unwrap().phase,
        MutationPhase::ProviderObserved
    );
    for action in [
        carosign_engine::RegistryAction::RecordNoChange,
        carosign_engine::RegistryAction::ApplyDesiredState,
    ] {
        let result =
            journal.commit_reconciliation(carosign_persistence::journal::CommitReconciliation {
                mutation,
                action,
                enrollment: Some(record(8).enrollment),
            });
        assert!(matches!(
            result,
            Err(carosign_persistence::model::StorageError::Unavailable)
        ));
        assert_eq!(journal.scope_lock(&scope).unwrap(), Some(mutation));
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_record_is_missing_not_corrupt() {
    let path = temp_path("missing");
    let store = RedbStore::create(&path).expect("создано");
    let reader = RegistryReader::new(&store);
    assert_eq!(
        reader
            .by_id(EnrollmentId::from_bytes([9u8; 16]).expect("валид"))
            .expect("читается"),
        Lookup::Missing
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn unreadable_record_is_corrupt_not_missing() {
    let path = temp_path("corrupt");
    let store = RedbStore::create(&path).expect("создано");
    {
        let mut tx = store.begin_write().expect("запись");
        tx.put(
            carosign_persistence::store::Table::Enrollments,
            &[6u8; 16],
            b"not a record",
        )
        .expect("положено");
        tx.commit_durable().expect("зафиксировано");
    }
    let reader = RegistryReader::new(&store);
    // ⚠️ "I didn't make it out"  ⁇  "no record."
    assert!(
        reader
            .by_id(EnrollmentId::from_bytes([6u8; 16]).expect("валид"))
            .is_err()
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn generation_is_read_from_exact_registration_and_authenticator() {
    use carosign_persistence::model::StorageError;
    use carosign_persistence::store::Table;
    let path = temp_path("generation");
    let store = RedbStore::create(&path).unwrap();
    let mut entry = record(7);
    entry.enrollment.generation = EnrollmentGeneration::new(4).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        entry.enrollment.id.as_bytes(),
        &enrollment_codec::encode(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let reader = RegistryReader::new(&store);
    let mut reference = entry.enrollment;
    reference.generation = EnrollmentGeneration::FIRST;
    assert_eq!(reader.generation_of(reference).unwrap(), Some(4));
    reference.authenticator = AuthenticatorId::from_bytes([9; 16]).unwrap();
    assert_eq!(
        reader.generation_of(reference),
        Err(StorageError::GenerationConflict)
    );
    reference.id = EnrollmentId::from_bytes([8; 16]).unwrap();
    assert_eq!(reader.generation_of(reference).unwrap(), None);
    let _ = std::fs::remove_file(path);
}

#[test]
fn registry_rejects_mismatched_primary_key_and_dangling_index() {
    use carosign_persistence::store::Table;
    let path = temp_path("bad-identity");
    let store = RedbStore::create(&path).unwrap();
    let entry = record(7);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        &[8; 16],
        &enrollment_codec::encode(&entry).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let reader = RegistryReader::new(&store);
    assert!(
        reader
            .by_id(EnrollmentId::from_bytes([8; 16]).unwrap())
            .is_err()
    );

    let key = carosign_persistence::redb::codec::enrollment_index_key(&entry);
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::EnrollmentIndex, &key, entry.enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    let mut scope = [0; 48];
    scope.copy_from_slice(&key[..48]);
    assert!(reader.by_scope(&scope).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::EnrollmentIndex, &key, &[8; 16]).unwrap();
    tx.commit_durable().unwrap();
    assert!(reader.by_scope(&scope).is_err());
    let _ = std::fs::remove_file(path);
}
