//! The order of starting the service: settings → recovery → listener.
//!
//! ⚠️ What is being tested is what breaks quietly: the service lets customers in before it
//! knows the closed areas; the damaged base is silently recreated; the setup extends the
//! limit beyond the ceiling.

use carosignd::bootstrap::{BootstrapError, Readiness, SchemaStatus, recover, recover_guarded};
use carosignd::config::{Config, ConfigError};
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("carosignd-{name}-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn defaults_are_conservative_and_valid() {
    let config = Config::default();
    assert!(config.validate().is_ok());
    assert_eq!(config.ops_per_connection, 4);
    assert_eq!(config.hello_timeout_s, 5);
}

#[test]
fn config_cannot_widen_ceilings() {
    assert!(matches!(
        Config::parse("ops_per_connection=64"),
        Err(ConfigError::OutOfBounds { .. })
    ));
    assert!(Config::parse("ops_per_connection=2").is_ok());
    assert!(matches!(
        Config::parse("socket=relative.sock"),
        Err(ConfigError::BadPath { .. })
    ));
    assert!(Config::parse("store=/one\nstore=/two").is_err());
    let invalid = Config {
        store: "relative.redb".into(),
        ..Config::default()
    };
    assert!(matches!(
        invalid.validate(),
        Err(ConfigError::BadPath { field: "store" })
    ));
}

#[test]
fn daemon_loads_explicit_config_before_opening_store() {
    let path = temp_path("config-store");
    let config_path = temp_path("config-file");
    let socket_path = temp_path("not-ready-socket");
    drop(carosign_persistence::redb::backend::RedbStore::create_current(&path).unwrap());
    std::fs::write(
        &config_path,
        format!(
            "store={}\nsocket={}\n",
            path.display(),
            socket_path.display()
        ),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("обязательные порты службы"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostic = stderr
        .lines()
        .find(|line| line.starts_with("component=carosignd "))
        .expect("ожидается структурированная диагностика запуска");
    assert!(diagnostic.contains("event=dependencies_unavailable outcome=unavailable"));
    assert!(!diagnostic.contains(&*path.to_string_lossy()));
    assert!(!diagnostic.contains(&*socket_path.to_string_lossy()));
    assert!(
        !socket_path.exists(),
        "слушатель нельзя публиковать до готовности"
    );
    std::fs::write(&config_path, "not_a_setting=1\n").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("незнакомое поле"));
    // The error string may contain the secret and control symbols of the terminal.
    std::fs::write(&config_path, "secret-marker\x1b[31m=1\n").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("незнакомое поле"));
    assert!(
        !output
            .stderr
            .windows(b"secret-marker".len())
            .any(|part| part == b"secret-marker")
    );
    assert!(!output.stderr.contains(&0x1b));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(config_path).unwrap();
}

#[test]
fn daemon_refuses_oversized_or_special_config_before_opening_store() {
    use std::os::unix::ffi::OsStrExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let config = temp_path("unsafe-config");
    std::fs::write(&config, vec![b'#'; 1024 * 1024 + 1]).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("файл настроек слишком велик"));
    std::fs::remove_file(&config).unwrap();

    let c_path = std::ffi::CString::new(config.as_os_str().as_bytes()).unwrap();
    // SAFETY: The path ends in zero and lives until the end of the call.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let mut child = Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("демон завис при открытии FIFO вместо отказа");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success());
    std::fs::remove_file(config).unwrap();
}

#[test]
fn daemon_refuses_unusable_socket_before_store_recovery() {
    let config_path = temp_path("invalid-socket-config");
    let store_path = temp_path("invalid-socket-store");
    let socket_path = format!("/{}", "s".repeat(200));
    std::fs::write(
        &config_path,
        format!("store={}\nsocket={}\n", store_path.display(), socket_path),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("поле socket: недопустимый путь"));
    assert!(!store_path.exists());
    std::fs::remove_file(config_path).unwrap();
}

#[test]
fn empty_store_without_schema_marker_is_not_ready() {
    // The blank log does not confirm the version of the scheme and the status of the
    // migration.
    let path = temp_path("empty");
    carosign_persistence::redb::backend::RedbStore::create(&path).expect("создано");
    let (_store, recovered, readiness) = recover(path.to_str().expect("путь")).expect("готово");
    assert!(recovered.blocked.is_empty());
    assert_eq!(recovered.schema, SchemaStatus::Unverified);
    assert_eq!(readiness, Readiness::NotReady);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn explicit_initialization_survives_reopen_without_pretending_service_is_ready() {
    use carosign_persistence::redb::backend::RedbStore;
    use carosign_persistence::store::{MetadataStore, Table};
    use std::os::unix::fs::PermissionsExt;
    let path = temp_path("v3-init");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("init-store")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let before = std::fs::read(&path).unwrap();
    assert!(
        !std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
            .arg("init-store")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (store, recovered, readiness) = recover_guarded(path.to_str().unwrap()).unwrap();
    assert_eq!(recovered.schema, SchemaStatus::V4);
    assert_eq!(readiness, Readiness::NotReady);
    assert!(recovered.blocked.is_empty());
    let mut tx = store.begin_write().unwrap();
    assert!(tx.put(Table::Meta, b"biometric.schema", b"fake").is_err());
    assert!(tx.delete(Table::Meta, b"biometric.schema").is_err());
    tx.abort();
    drop(store);
    let reopened = RedbStore::open_current_guarded(&path).unwrap();
    assert!(!reopened.has_schema_v3().unwrap());
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn v4_prepared_closes_on_startup_only_after_resource_check() {
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, CommandId, EnrollmentId, IdempotencyKey, MutationId,
        OperationId, SubjectId, WorkerInstanceId,
    };
    use carosign_persistence::journal::{ArmMutation, PrepareMutation};
    use carosign_persistence::model::IdempotencyRecord;
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
    use carosign_persistence::store::MetadataStore;
    let crash_path = std::env::var_os("CAROSIGN_TEST_CRASH_ARMED_STORE").map(PathBuf::from);
    let path = crash_path
        .clone()
        .unwrap_or_else(|| temp_path("v4-prepared-scope"));
    let socket = path.with_extension("sock");
    let config = path.with_extension("conf");
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let mutation = MutationId::from_bytes([4; 16]).unwrap();
    let operation = OperationId::from_bytes([5; 16]).unwrap();
    let input = PrepareMutation {
        mutation,
        operation,
        subject,
        authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([6; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject,
                authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [7; 32],
        key: IdempotencyKey::from_bytes([8; 16]).unwrap(),
        request_digest: [9; 32],
        operation,
        mutation: Some(mutation),
    };
    let store = carosign_persistence::redb::backend::RedbStore::create_current(&path).unwrap();
    let mut tx = store.begin_write().unwrap();
    carosign_persistence::mutation_store::stage_prepared_v4(&mut *tx, &input, &request).unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    let (store, recovered, readiness) = recover_guarded(path.to_str().unwrap()).unwrap();
    assert_eq!(readiness, Readiness::NotReady);
    assert_eq!(recovered.blocked, vec![mutation]);
    assert_eq!(recovered.blocked_scopes.len(), 1);
    assert_eq!(recovered.blocked_scopes[0].subject, subject);
    assert_eq!(recovered.blocked_scopes[0].authenticator, authenticator);
    let coordinator = recovered.coordinator().unwrap();
    assert!(
        coordinator.is_blocked(carosign_runtime::recovery::blocked_scope::RecoveryScope {
            subject,
            authenticator,
        })
    );
    drop(store);
    std::fs::write(
        &config,
        format!("store={}\nsocket={}\n", path.display(), socket.display()),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!socket.exists());
    use carosign_persistence::model::MutationTerminalOutcome;
    use carosign_persistence::mutation_store::{read_completed_v4, recover_unarmed_v4};
    let (store, recovered, _) = recover_guarded(path.to_str().unwrap()).unwrap();
    assert_eq!(recovered.blocked, vec![mutation]);
    assert_eq!(recover_unarmed_v4(&store, |_| false).unwrap(), 1);
    assert!(read_completed_v4(&store, mutation, operation).is_err());
    assert_eq!(
        recover_unarmed_v4(&store, |candidate| candidate == operation).unwrap(),
        0
    );
    drop(store);

    let (store, recovered, _) = recover_guarded(path.to_str().unwrap()).unwrap();
    assert!(recovered.blocked.is_empty());
    recover_unarmed_v4(&store, |_| true).unwrap();
    assert_eq!(
        read_completed_v4(&store, mutation, operation)
            .unwrap()
            .terminal_outcome,
        Some(MutationTerminalOutcome::NoChange)
    );
    assert!(
        store
            .begin_read()
            .unwrap()
            .scan_prefix(carosign_persistence::store::Table::Enrollments, &[])
            .unwrap()
            .is_empty()
    );
    let next = PrepareMutation {
        mutation: MutationId::from_bytes([10; 16]).unwrap(),
        operation: OperationId::from_bytes([11; 16]).unwrap(),
        ..input
    };
    let next_request = IdempotencyRecord {
        key: IdempotencyKey::from_bytes([12; 16]).unwrap(),
        operation: next.operation,
        mutation: Some(next.mutation),
        ..request
    };
    let mut tx = store.begin_write().unwrap();
    carosign_persistence::mutation_store::stage_prepared_v4(&mut *tx, &next, &next_request)
        .unwrap();
    let arm = ArmMutation {
        mutation: next.mutation,
        worker: WorkerInstanceId::from_bytes([13; 16]).unwrap(),
        epoch: carosign_core::device::DeviceEpoch::FIRST,
        command: CommandId::from_bytes([14; 16]).unwrap(),
        recovery_ref: None,
    };
    let effect = carosign_engine::PlannedEffect::at(
        carosign_engine::EffectSpec::SendMutatingCommand,
        carosign_engine::TransitionIndex::new(7),
        0,
    );
    carosign_persistence::mutation_store::stage_arm_v4(&mut *tx, &arm, effect).unwrap();
    tx.commit_durable().unwrap();
    if let Some(crash_path) = crash_path {
        std::fs::write(crash_path.with_extension("armed"), b"armed").unwrap();
        loop {
            std::thread::park();
        }
    }
    drop(store);

    let (store, recovered, _) = recover_guarded(path.to_str().unwrap()).unwrap();
    assert_eq!(recovered.blocked, vec![next.mutation]);
    assert_eq!(recover_unarmed_v4(&store, |_| true).unwrap(), 1);
    assert!(read_completed_v4(&store, next.mutation, next.operation).is_err());
    drop(store);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(config).unwrap();
}

#[test]
fn crash_after_v4_dispatch_armed_preserves_the_scope_and_command() {
    use carosign_core::ids::{CommandId, MutationId, OperationId};
    use carosign_persistence::model::MutationPhase;
    use carosign_persistence::mutation_store::{read_mutation_by_operation_v4, recover_unarmed_v4};

    let path = temp_path("crash-after-armed");
    let marker = path.with_extension("armed");
    let _ = std::fs::remove_file(&marker);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "v4_prepared_closes_on_startup_only_after_resource_check",
        ])
        .env("CAROSIGN_TEST_CRASH_ARMED_STORE", &path)
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !marker.exists() && std::time::Instant::now() < deadline {
        assert!(child.try_wait().unwrap().is_none());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.kill().unwrap();
    let _ = child.wait().unwrap();
    assert!(
        marker.exists(),
        "writer did not reach durable DispatchArmed"
    );

    let (store, recovered, _) = recover_guarded(path.to_str().unwrap()).unwrap();
    let mutation = MutationId::from_bytes([10; 16]).unwrap();
    let operation = OperationId::from_bytes([11; 16]).unwrap();
    assert_eq!(recovered.blocked, [mutation]);
    assert_eq!(recovered.blocked_scopes.len(), 1);
    recover_unarmed_v4(&store, |_| true).unwrap();
    let record = read_mutation_by_operation_v4(&store, operation)
        .unwrap()
        .unwrap();
    assert_eq!(record.phase, MutationPhase::DispatchArmed);
    assert_eq!(
        record.command,
        Some(CommandId::from_bytes([14; 16]).unwrap())
    );
    assert!(record.terminal_outcome.is_none());
    drop(store);

    for file in [marker, path.with_extension("conf"), path] {
        std::fs::remove_file(file).unwrap();
    }
}

#[test]
fn guarded_v4_refuses_nonempty_or_unknown_without_modification() {
    use carosign_persistence::redb::backend::RedbStore;
    use redb::{Database, TableDefinition};
    const PACKAGES: TableDefinition<&[u8], &[u8]> = TableDefinition::new("packages");
    const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta");
    for unknown in [false, true] {
        let path = temp_path(if unknown { "v4-unknown" } else { "v4-nonempty" });
        drop(RedbStore::create_current(&path).unwrap());
        let db = Database::open(&path).unwrap();
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx
                .open_table(if unknown { META } else { PACKAGES })
                .unwrap();
            table.insert(&b"fixture"[..], &b"value"[..]).unwrap();
        }
        tx.commit().unwrap();
        drop(db);
        let before = std::fs::read(&path).unwrap();
        assert!(RedbStore::open_current_guarded(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn future_storage_revision_is_refused_without_modifying_database() {
    use carosign_persistence::redb::backend::RedbStore;
    use redb::{Database, TableDefinition};
    const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta");
    let path = temp_path("future-schema-unchanged");
    drop(RedbStore::create_current(&path).unwrap());
    let db = Database::open(&path).unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(META).unwrap();
        table
            .insert(&b"biometric.schema"[..], &b"BSC1\0\0\0\x05\0\0\0\0"[..])
            .unwrap();
    }
    tx.commit().unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(RedbStore::open_current_guarded(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unknown_or_corrupt_markers_are_preserved_and_do_not_prove_schema() {
    use redb::{Database, TableDefinition};
    const META: TableDefinition<&[u8], &[u8]> = TableDefinition::new("meta");
    for (name, marker) in [
        ("bad-magic", &b"BAD1\0\0\0\x03\0\0\0\0"[..]),
        ("bad-length", &b"BSC1"[..]),
        ("bad-version", &b"BSC1\0\0\0\x04\0\0\0\0"[..]),
        ("migrating", &b"BSC1\0\0\0\x03\0\0\0\x01"[..]),
    ] {
        let path = temp_path(name);
        drop(carosign_persistence::redb::backend::RedbStore::create_v3(&path).unwrap());
        {
            let db = Database::open(&path).unwrap();
            let tx = db.begin_write().unwrap();
            {
                let mut meta = tx.open_table(META).unwrap();
                meta.insert(&b"biometric.schema"[..], marker).unwrap();
            }
            tx.commit().unwrap();
        }
        let (store, recovered, readiness) = recover(path.to_str().unwrap()).unwrap();
        assert_eq!(recovered.schema, SchemaStatus::Unverified);
        assert_eq!(readiness, Readiness::NotReady);
        use carosign_persistence::store::{MetadataStore, Table};
        let tx = store.begin_read().unwrap();
        assert_eq!(
            tx.get(Table::Meta, b"biometric.schema").unwrap().as_deref(),
            Some(marker)
        );
        drop(tx);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn v3_marker_does_not_hide_a_corrupt_v3_journal() {
    use carosign_persistence::store::{MetadataStore, Table};
    let path = temp_path("v3-corrupt-journal");
    let store = carosign_persistence::redb::backend::RedbStore::create_v3(&path).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Mutations, &[1; 16], b"invalid").unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    assert!(matches!(
        recover(path.to_str().unwrap()),
        Err(BootstrapError::Scan(_))
    ));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn v3_recovery_preserves_the_exact_blocked_scope() {
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, EnrollmentId, IdempotencyKey, MutationId, OperationId,
        SubjectId,
    };
    use carosign_persistence::journal::{ArmMutation, MutationRepository, PrepareMutation};
    use carosign_persistence::model::IdempotencyRecord;
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
    use carosign_persistence::store::MetadataStore;
    let path = temp_path("v3-blocked-scope");
    let store = carosign_persistence::redb::backend::RedbStore::create_v3(&path).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let mutation = MutationId::from_bytes([4; 16]).unwrap();
    let operation = OperationId::from_bytes([5; 16]).unwrap();
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([6; 16]).unwrap()),
        1,
        Vec::new(),
        Scope {
            subject,
            authenticator,
        },
    )
    .unwrap();
    let input = PrepareMutation {
        mutation,
        operation,
        subject,
        authenticator,
        expected_generation: None,
        enroll: true,
        plan,
    };
    let request = IdempotencyRecord {
        caller_digest: [7; 32],
        key: IdempotencyKey::from_bytes([8; 16]).unwrap(),
        request_digest: [9; 32],
        operation,
        mutation: Some(mutation),
    };
    let unmarked = carosign_persistence::journal_store::StoreJournal::new(
        carosign_persistence::store::tests_support::MemoryStore::default(),
    );
    assert!(unmarked.prepare(input.clone()).is_err());
    assert!(
        unmarked
            .store()
            .begin_read()
            .unwrap()
            .scan_prefix(carosign_persistence::store::Table::Mutations, &[])
            .unwrap()
            .is_empty()
    );
    let journal = carosign_persistence::journal_store::StoreJournal::new(store);
    // A temporary journal should not put its format in the labeled database.
    assert!(journal.prepare(input.clone()).is_err());
    assert!(
        journal
            .store()
            .begin_read()
            .unwrap()
            .scan_prefix(carosign_persistence::store::Table::Mutations, &[])
            .unwrap()
            .is_empty()
    );
    let mut tx = journal.store().begin_write().unwrap();
    carosign_persistence::mutation_records::stage_prepared(&mut *tx, &input, &request).unwrap();
    tx.commit_durable().unwrap();
    assert!(journal.mark_uncertain(mutation).is_err());
    let arm = ArmMutation {
        mutation,
        worker: carosign_core::ids::WorkerInstanceId::from_bytes([10; 16]).unwrap(),
        epoch: carosign_core::device::DeviceEpoch::FIRST,
        command: carosign_core::ids::CommandId::from_bytes([11; 16]).unwrap(),
        recovery_ref: None,
    };
    let mut tx = journal.store().begin_write().unwrap();
    carosign_persistence::mutation_records::stage_arm(
        &mut *tx,
        &arm,
        carosign_engine::PlannedEffect::at(
            carosign_engine::EffectSpec::SendMutatingCommand,
            carosign_engine::TransitionIndex::new(1),
            0,
        ),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    journal.mark_uncertain(mutation).unwrap();
    assert_eq!(
        journal.record(mutation).unwrap().phase,
        carosign_persistence::model::MutationPhase::NeedsReconciliation
    );
    assert_eq!(journal.scan_incomplete().unwrap().len(), 1);
    let snapshot = carosign_persistence::recovery::read_mutation_snapshot_v3(
        journal.store(),
        mutation,
        operation,
    )
    .unwrap();
    assert_eq!(
        snapshot.mutation.phase,
        carosign_persistence::model::MutationPhase::NeedsReconciliation
    );
    assert_eq!(
        snapshot.outbox[0].phase,
        carosign_persistence::outbox::EffectPhase::ReconciliationRequired
    );
    drop(journal);
    let (store, recovered, readiness) = recover(path.to_str().unwrap()).unwrap();
    assert_eq!(readiness, Readiness::NotReady);
    assert_eq!(recovered.blocked, vec![mutation]);
    assert_eq!(recovered.blocked_scopes.len(), 1);
    assert_eq!(recovered.blocked_scopes[0].mutation, mutation);
    assert_eq!(recovered.blocked_scopes[0].subject, subject);
    assert_eq!(recovered.blocked_scopes[0].authenticator, authenticator);
    let coordinator = recovered.coordinator().unwrap();
    assert!(
        coordinator.is_blocked(carosign_runtime::recovery::blocked_scope::RecoveryScope {
            subject,
            authenticator
        })
    );
    assert!(
        !coordinator.is_blocked(carosign_runtime::recovery::blocked_scope::RecoveryScope {
            subject: SubjectRef {
                authority: subject.authority,
                subject: SubjectId::from_bytes([99; 16]).unwrap()
            },
            authenticator
        })
    );
    drop(store);

    // We check the executable demon: the unfinished sending should not disappear at launch,
    // and the client socket should appear without other trusted dependencies and confirmation
    // of recovery.
    let config_path = temp_path("v3-blocked-config");
    let socket_path = temp_path("v3-blocked-socket");
    std::fs::write(
        &config_path,
        format!(
            "store={}\nsocket={}\n",
            path.display(),
            socket_path.display()
        ),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"))
        .arg("--config")
        .arg(&config_path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("неизвестный состав таблиц v4"));
    assert!(!socket_path.exists());
    let reopened = carosign_persistence::redb::backend::RedbStore::open(&path).unwrap();
    let after =
        carosign_persistence::recovery::read_mutation_snapshot_v3(&reopened, mutation, operation)
            .unwrap();
    assert_eq!(after.mutation.phase, snapshot.mutation.phase);
    assert_eq!(after.outbox[0].phase, snapshot.outbox[0].phase);
    drop(reopened);
    std::fs::remove_file(config_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unknown_metadata_never_proves_schema_and_is_preserved() {
    use carosign_persistence::redb::backend::RedbStore;
    use carosign_persistence::store::{MetadataStore, Table};
    let path = temp_path("unknown-schema");
    {
        let store = RedbStore::create(&path).unwrap();
        let mut tx = store.begin_write().unwrap();
        tx.put(Table::Meta, b"unknown", b"opaque marker").unwrap();
        tx.commit_durable().unwrap();
    }
    let (store, recovered, readiness) = recover(path.to_str().unwrap()).unwrap();
    assert_eq!(recovered.schema, SchemaStatus::Unverified);
    assert_eq!(readiness, Readiness::NotReady);
    let tx = store.begin_read().unwrap();
    assert_eq!(
        tx.get(Table::Meta, b"unknown").unwrap().as_deref(),
        Some(b"opaque marker".as_slice())
    );
    drop(tx);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn corrupt_store_stops_the_service() {
    let path = temp_path("corrupt");
    std::fs::write(&path, b"not a redb database").expect("мусор");
    assert!(matches!(
        recover(path.to_str().expect("путь")),
        Err(BootstrapError::Store(_))
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn legacy_prepared_mutation_is_quarantined() {
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, EnrollmentId, MutationId, OperationId, SubjectId,
    };
    use carosign_persistence::journal::{MutationRepository, PrepareMutation};
    use carosign_persistence::journal_store::StoreJournal;
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};

    let path = temp_path("prepared");
    let mutation = MutationId::from_bytes([1; 16]).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([3; 16]).unwrap(),
        subject: SubjectId::from_bytes([4; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([5; 16]).unwrap();
    {
        let store = carosign_persistence::redb::backend::RedbStore::create(&path).unwrap();
        let journal = StoreJournal::legacy_fixture(store);
        let plan = MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([8; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject,
                authenticator,
            },
        )
        .unwrap();
        journal
            .prepare(PrepareMutation {
                mutation,
                operation: OperationId::from_bytes([2; 16]).unwrap(),
                subject,
                authenticator,
                expected_generation: None,
                enroll: true,
                plan,
            })
            .unwrap();
    }
    let (_store, recovered, readiness) = recover(path.to_str().unwrap()).unwrap();
    assert_eq!(recovered.blocked, vec![mutation]);
    assert_eq!(recovered.needs_decision, vec![mutation]);
    assert_eq!(readiness, Readiness::NotReady);
    let _ = std::fs::remove_file(path);
}

#[test]
fn legacy_journal_refuses_reused_operation_across_scopes() {
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, EnrollmentId, MutationId, OperationId, SubjectId,
    };
    use carosign_persistence::journal::{MutationRepository, PrepareMutation};
    use carosign_persistence::journal_store::StoreJournal;
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
    let path = temp_path("duplicate-operation");
    let store = carosign_persistence::redb::backend::RedbStore::create(&path).unwrap();
    let journal = StoreJournal::legacy_fixture(store);
    for byte in [1u8, 2] {
        let subject = SubjectRef {
            authority: AuthorityId::from_bytes([3; 16]).unwrap(),
            subject: SubjectId::from_bytes([byte; 16]).unwrap(),
        };
        let authenticator = AuthenticatorId::from_bytes([5; 16]).unwrap();
        let result = journal.prepare(PrepareMutation {
            mutation: MutationId::from_bytes([byte; 16]).unwrap(),
            operation: OperationId::from_bytes([9; 16]).unwrap(),
            subject,
            authenticator,
            expected_generation: None,
            enroll: true,
            plan: MutationPlan::build(
                Transition::EnrollNew,
                None,
                Some(EnrollmentId::from_bytes([byte; 16]).unwrap()),
                1,
                Vec::new(),
                Scope {
                    subject,
                    authenticator,
                },
            )
            .unwrap(),
        });
        assert_eq!(result.is_ok(), byte == 1);
    }
    drop(journal);
    let (_, recovered, readiness) = recover(path.to_str().unwrap()).unwrap();
    assert_eq!(recovered.blocked.len(), 1);
    assert_eq!(readiness, Readiness::NotReady);
    let _ = std::fs::remove_file(path);
}

#[test]
fn legacy_armed_mutation_is_quarantined() {
    use carosign_core::device::DeviceEpoch;
    use carosign_core::enrollment::EnrollmentGeneration;
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, CommandId, MutationId, OperationId, SubjectId,
        WorkerInstanceId,
    };
    use carosign_persistence::journal::{ArmMutation, MutationRepository, PrepareMutation};
    use carosign_persistence::journal_store::StoreJournal;

    let path = temp_path("armed");
    {
        let store = carosign_persistence::redb::backend::RedbStore::create(&path).expect("создано");
        let journal = StoreJournal::legacy_fixture(store);
        let mutation = MutationId::from_bytes([1u8; 16]).expect("валид");
        journal
            .prepare(PrepareMutation {
                mutation,
                operation: OperationId::from_bytes([2u8; 16]).expect("валид"),
                subject: SubjectRef {
                    authority: AuthorityId::from_bytes([3u8; 16]).expect("валид"),
                    subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
                },
                authenticator: AuthenticatorId::from_bytes([5u8; 16]).expect("валид"),
                expected_generation: Some(EnrollmentGeneration::FIRST),
                enroll: true,
                plan: carosign_persistence::mutation_plan::MutationPlan::build(
                    carosign_persistence::mutation_plan::Transition::EnrollNew,
                    None,
                    Some(carosign_core::ids::EnrollmentId::from_bytes([8u8; 16]).expect("валид")),
                    1,
                    Vec::new(),
                    carosign_persistence::mutation_plan::Scope {
                        subject: SubjectRef {
                            authority: AuthorityId::from_bytes([3u8; 16]).expect("валид"),
                            subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
                        },
                        authenticator: AuthenticatorId::from_bytes([5u8; 16]).expect("валид"),
                    },
                )
                .expect("план"),
            })
            .expect("готовится");
        journal
            .arm(ArmMutation {
                mutation,
                worker: WorkerInstanceId::from_bytes([6u8; 16]).expect("валид"),
                epoch: DeviceEpoch::FIRST,
                command: CommandId::from_bytes([7u8; 16]).expect("валид"),
                recovery_ref: None,
            })
            .expect("разрешено");
    }
    {
        use carosign_persistence::store::{MetadataStore, Table};
        let store = carosign_persistence::redb::backend::RedbStore::open(&path).unwrap();
        let journal = StoreJournal::new(store);
        let mutation = MutationId::from_bytes([1; 16]).unwrap();
        let before = journal
            .store()
            .begin_read()
            .unwrap()
            .get(Table::Mutations, mutation.as_bytes())
            .unwrap();
        assert!(journal.mark_uncertain(mutation).is_err());
        let after = journal
            .store()
            .begin_read()
            .unwrap()
            .get(Table::Mutations, mutation.as_bytes())
            .unwrap();
        assert_eq!(after, before);
    }
    let (_store, recovered, readiness) = recover(path.to_str().expect("путь")).expect("скан");
    assert_eq!(recovered.blocked.len(), 1);
    assert_eq!(recovered.needs_decision, recovered.blocked);
    assert_eq!(readiness, Readiness::NotReady);
    use carosign_persistence::store::{MetadataStore, Table};
    let mut tx = _store.begin_write().unwrap();
    let locks = tx.scan_prefix(Table::MutationScopeLock, &[]).unwrap();
    assert_eq!(locks.len(), 1);
    tx.delete(Table::MutationScopeLock, &locks[0].0).unwrap();
    tx.commit_durable().unwrap();
    drop(_store);
    assert!(matches!(
        recover(path.to_str().unwrap()),
        Err(BootstrapError::Scan(_))
    ));
    let _ = std::fs::remove_file(&path);
}
#[test]
fn quarantine_command_refuses_unprivileged_process_before_opening_files() {
    if unsafe { libc::getuid() } == 0 {
        return;
    }
    let missing =
        std::env::temp_dir().join(format!("carosign-install-refuse-{}", std::process::id()));
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_carosignd"));
    command.arg("install-quarantine");
    for _ in 0..9 {
        command.arg(&missing);
    }
    let result = command.arg("versions/test").output().unwrap();
    assert!(!result.status.success());
    assert!(!missing.exists());
}

#[path = "service_boot_fixtures/armed_recovery.tests.rs"]
mod armed_recovery;
