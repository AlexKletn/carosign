//! Storage on redb: The log goes through the same checks as the stand and **plus** checks the
//! real record on the disc.
//!
//! ⚠️ This is a test of what the backroom can't prove: `commit_durable` The record is read
//! from a rediscovered database. If it's not there, "sustainable" was a word, not a fact.
//!
//! ⚠️ There are no power failure tests here: They need a power-off stand, and this is a
//! separate job, she said in the report. `NOT RUN`.

use carosign_core::device::DeviceEpoch;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, MutationId, OperationId, SubjectId, WorkerInstanceId,
};
use carosign_persistence::journal::{MutationRepository, PrepareMutation};
use carosign_persistence::journal_store::StoreJournal;
use carosign_persistence::model::{MutationClosure, MutationPhase, StorageError};
use carosign_persistence::recovery::{RecoveryDecision, StartupScan, decide_for};
use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::redb::codec::{Envelope, SCHEMA_VERSION};
use carosign_persistence::store::{MetadataStore, Table};
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("carosign-test-{name}-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn mutation() -> MutationId {
    MutationId::from_bytes([4u8; 16]).expect("валид")
}

fn operation() -> OperationId {
    OperationId::from_bytes([5u8; 16]).expect("валид")
}

fn prepare_input() -> PrepareMutation {
    PrepareMutation {
        mutation: mutation(),
        operation: operation(),
        subject: subject(),
        authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
        expected_generation: None,
        enroll: true,
        plan: plan_for(subject(), authenticator()),
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
fn write_survives_reopening_the_database() {
    // ⚠️ Real stability check: after fixing the base is closed and reopened - the record must
    // be in place.
    let path = temp_path("survive");
    {
        let store = RedbStore::create(&path).expect("создано");
        let mut tx = store.begin_write().expect("запись");
        tx.put(Table::Meta, b"schema", b"1").expect("положено");
        let receipt = tx.commit_durable().expect("зафиксировано");
        assert_eq!(format!("{receipt:?}"), "DurableReceipt(подтверждено)");
    }
    {
        let store = RedbStore::open(&path).expect("открыто");
        let tx = store.begin_read().expect("чтение");
        let value = tx.get(Table::Meta, b"schema").expect("читается");
        assert_eq!(value.as_deref(), Some(&b"1"[..]));
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn reopen_preserves_committed_phase_and_scope_after_uncertain_marker() {
    for (name, phase) in [
        ("committed-phase", MutationPhase::RegistryCommitted),
        ("no-change-phase", MutationPhase::NoChange),
    ] {
        let path = temp_path(name);
        let expected;
        {
            let journal = StoreJournal::legacy_fixture(RedbStore::create(&path).unwrap());
            journal.prepare(prepare_input()).unwrap();
            journal
                .arm(carosign_persistence::journal::ArmMutation {
                    mutation: mutation(),
                    worker: WorkerInstanceId::from_bytes([7; 16]).unwrap(),
                    epoch: DeviceEpoch::FIRST,
                    command: CommandId::from_bytes([6; 16]).unwrap(),
                    recovery_ref: None,
                })
                .unwrap();
            let mut record = journal.record(mutation()).unwrap();
            record.phase = phase;
            expected = carosign_persistence::redb::record::encode_mutation(&record).unwrap();
            let mut tx = journal.store().begin_write().unwrap();
            tx.put(Table::Mutations, mutation().as_bytes(), &expected)
                .unwrap();
            tx.commit_durable().unwrap();
            assert!(journal.mark_uncertain(mutation()).is_err());
        }
        {
            let journal = StoreJournal::legacy_fixture(RedbStore::open(&path).unwrap());
            let scan = StartupScan::scan(journal.store()).unwrap();
            assert!(
                scan.needing_reconciliation.contains(&mutation())
                    || scan.needs_decision.contains(&mutation())
            );
            let tx = journal.store().begin_read().unwrap();
            assert_eq!(
                tx.get(Table::Mutations, mutation().as_bytes()).unwrap(),
                Some(expected)
            );
            assert_eq!(
                tx.get(
                    Table::MutationScopeLock,
                    &journal.record(mutation()).unwrap().scope_key()
                )
                .unwrap()
                .as_deref(),
                Some(mutation().as_bytes().as_slice())
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn create_never_initializes_an_existing_file_or_repairs_an_incomplete_database() {
    let path = temp_path("create-existing");
    std::fs::write(&path, []).unwrap();
    assert!(RedbStore::create(&path).is_err());
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
    std::fs::remove_file(&path).unwrap();

    drop(redb::Database::create(&path).unwrap());
    assert!(RedbStore::create(&path).is_err());
    assert!(RedbStore::open(&path).is_err());
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn aborted_transaction_leaves_nothing() {
    let path = temp_path("abort");
    let store = RedbStore::create(&path).expect("создано");
    {
        let mut tx = store.begin_write().expect("запись");
        tx.put(Table::Meta, b"ghost", b"1").expect("положено");
        tx.abort();
    }
    let read = store.begin_read().expect("чтение");
    assert_eq!(read.get(Table::Meta, b"ghost").expect("читается"), None);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn journal_prepare_and_arm_are_durable() {
    let path = temp_path("journal");
    {
        let store = RedbStore::create(&path).expect("создано");
        let journal = StoreJournal::legacy_fixture(store);
        journal.prepare(prepare_input()).expect("готовится");
        let ticket = journal
            .arm(carosign_persistence::journal::ArmMutation {
                mutation: mutation(),
                worker: WorkerInstanceId::from_bytes([7u8; 16]).expect("валид"),
                epoch: DeviceEpoch::FIRST,
                command: CommandId::from_bytes([6u8; 16]).expect("валид"),
                recovery_ref: None,
            })
            .expect("разрешено");
        assert_eq!(ticket.mutation(), mutation());
    }
    {
        // After reopening, the magazine sees the same** state.
        let store = RedbStore::open(&path).expect("открыто");
        let journal = StoreJournal::legacy_fixture(store);
        let record = journal.record(mutation()).expect("запись есть");
        assert_eq!(record.phase, MutationPhase::DispatchArmed);
        assert_eq!(
            decide_for(&record),
            RecoveryDecision::BlockAndReconcile,
            "запись после разрешения обязана требовать сверки"
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn unproven_close_leaves_scope_locked_after_reopen() {
    let path = temp_path("closed");
    {
        let store = RedbStore::create(&path).expect("создано");
        let journal = StoreJournal::legacy_fixture(store);
        journal.prepare(prepare_input()).expect("готовится");
        journal
            .arm(carosign_persistence::journal::ArmMutation {
                mutation: mutation(),
                worker: WorkerInstanceId::from_bytes([7; 16]).unwrap(),
                epoch: DeviceEpoch::FIRST,
                command: CommandId::from_bytes([6; 16]).unwrap(),
                recovery_ref: None,
            })
            .unwrap();
        assert!(
            journal
                .finalize(mutation(), MutationClosure::NoChange)
                .is_err()
        );
    }
    {
        let store = RedbStore::open(&path).expect("открыто");
        let journal = StoreJournal::legacy_fixture(store);
        let scan = StartupScan::scan(journal.store()).expect("скан");
        assert_eq!(scan.needing_reconciliation, vec![mutation()]);
        let record = journal.record(mutation()).unwrap();
        assert_eq!(
            journal.scope_lock(&record.scope_key()).unwrap(),
            Some(mutation())
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn scope_lock_prevents_second_prepare() {
    let path = temp_path("lock");
    let store = RedbStore::create(&path).expect("создано");
    let journal = StoreJournal::legacy_fixture(store);
    journal.prepare(prepare_input()).expect("готовится");
    let second = journal.prepare(PrepareMutation {
        mutation: MutationId::from_bytes([9u8; 16]).expect("валид"),
        ..prepare_input()
    });
    assert_eq!(second.unwrap_err(), StorageError::GenerationConflict);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn envelope_round_trips_through_redb() {
    let path = temp_path("envelope");
    let store = RedbStore::create(&path).expect("создано");
    let payload = b"record";
    let bytes = Envelope {
        schema: SCHEMA_VERSION,
        payload: payload.to_vec(),
    }
    .encode()
    .expect("кодируется");
    {
        let mut tx = store.begin_write().expect("запись");
        tx.put(Table::Enrollments, &[1u8; 16], &bytes)
            .expect("положено");
        tx.commit_durable().expect("зафиксировано");
    }
    let read = store.begin_read().expect("чтение");
    let value = read
        .get(Table::Enrollments, &[1u8; 16])
        .expect("читается")
        .expect("есть");
    let decoded = Envelope::decode(&value).expect("разбирается");
    assert_eq!(decoded.payload, payload);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn corrupt_database_is_not_recreated() {
    // ⚠️ The damaged database is not recreated: "empty" instead of damaged - the loss of all
    // registrations, issued as a blank sheet.
    let path = temp_path("corrupt");
    std::fs::write(&path, b"this is not a redb database").expect("запись мусора");
    let result = RedbStore::open(&path);
    assert!(result.is_err());
    assert!(std::fs::read(&path).is_ok(), "файл обязан остаться");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn opening_incomplete_database_does_not_create_missing_tables() {
    let path = temp_path("missing-table");
    {
        let db = redb::Database::create(&path).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx
                .open_table(redb::TableDefinition::<&[u8], &[u8]>::new("meta"))
                .unwrap();
            table.insert(&b"existing"[..], &b"unchanged"[..]).unwrap();
        }
        tx.commit().unwrap();
    }
    assert!(matches!(
        RedbStore::open(&path),
        Err(StorageError::Corrupt { .. })
    ));
    {
        let db = redb::Database::open(&path).unwrap();
        let tx = db.begin_read().unwrap();
        let meta = tx
            .open_table(redb::TableDefinition::<&[u8], &[u8]>::new("meta"))
            .unwrap();
        assert_eq!(
            meta.get(&b"existing"[..]).unwrap().unwrap().value(),
            b"unchanged"
        );
        assert!(
            tx.open_table(redb::TableDefinition::<&[u8], &[u8]>::new("mutations"))
                .is_err()
        );
    }
    std::fs::remove_file(path).unwrap();
}

/// A separate process to check recovery from a sudden cliff.
#[test]
fn crash_recovery_child() {
    let Ok(path) = std::env::var("CAROSIGN_TEST_CRASH_DB") else {
        return;
    };
    let marker = std::env::var("CAROSIGN_TEST_CRASH_MARKER").unwrap();
    let stage = std::env::var("CAROSIGN_TEST_CRASH_STAGE").unwrap();
    let journal =
        StoreJournal::legacy_fixture(RedbStore::open(std::path::Path::new(&path)).unwrap());
    journal.prepare(prepare_input()).unwrap();
    if stage == "armed" {
        let _ticket = journal
            .arm(carosign_persistence::journal::ArmMutation {
                mutation: mutation(),
                worker: WorkerInstanceId::from_bytes([7; 16]).unwrap(),
                epoch: DeviceEpoch::FIRST,
                command: CommandId::from_bytes([6; 16]).unwrap(),
                recovery_ref: None,
            })
            .unwrap();
    } else {
        assert_eq!(stage, "prepared");
    }
    std::fs::write(marker, b"durable").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn killed_process_recovers_prepared_and_armed_without_replay() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    for stage in ["prepared", "armed"] {
        let path = temp_path(stage);
        let marker = path.with_extension("ready");
        let _ = std::fs::remove_file(&marker);
        drop(RedbStore::create(&path).expect("создано"));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_recovery_child", "--nocapture"])
            .env("CAROSIGN_TEST_CRASH_DB", &path)
            .env("CAROSIGN_TEST_CRASH_MARKER", &marker)
            .env("CAROSIGN_TEST_CRASH_STAGE", stage)
            .stdout(Stdio::null())
            .spawn()
            .expect("дочерний процесс");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let reached = marker.exists();
        child.kill().expect("остановка процесса");
        child.wait().expect("дождаться завершения");
        assert!(reached, "дочерний процесс не достиг {stage}");

        let store = RedbStore::open(&path).expect("база после обрыва");
        let journal = StoreJournal::legacy_fixture(store);
        let record = journal.record(mutation()).expect("журнал сохранился");
        let scan = StartupScan::scan(journal.store()).expect("скан после обрыва");
        assert!(!carosign_persistence::recovery::may_replay(&record));
        if stage == "prepared" {
            assert_eq!(record.phase, MutationPhase::Prepared);
            assert_eq!(scan.safe_to_close, vec![mutation()]);
            assert!(scan.needing_reconciliation.is_empty());
        } else {
            assert_eq!(record.phase, MutationPhase::DispatchArmed);
            assert_eq!(scan.needing_reconciliation, vec![mutation()]);
            assert!(scan.safe_to_close.is_empty());
        }
        assert!(matches!(
            journal.prepare(PrepareMutation {
                mutation: MutationId::from_bytes([9; 16]).unwrap(),
                ..prepare_input()
            }),
            Err(StorageError::GenerationConflict)
        ));
        drop(journal);
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&marker).unwrap();
    }
}
