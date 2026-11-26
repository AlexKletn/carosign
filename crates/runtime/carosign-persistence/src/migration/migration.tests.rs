use super::*;
use crate::redb::backend::RedbStore;
use crate::store::{MetadataStore, Table};
use redb::TableDefinition;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn database() -> (std::path::PathBuf, redb::Database) {
    let path = std::env::temp_dir().join(format!(
        "carosign-migration-preflight-{}-{}.redb",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    drop(RedbStore::create_v3(&path).unwrap());
    let db = redb::Database::open(&path).unwrap();
    (path, db)
}

#[test]
fn verified_backup_retains_lock_and_detects_mutation_or_unlink() {
    use crate::redb::backend::VerifiedV3Backup;
    use std::os::unix::fs::FileExt;
    let (path, db) = database();
    drop(db);
    let original = std::fs::read(&path).unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let retained = file.try_clone().unwrap();
    let verified = VerifiedV3Backup::inspect(file, 16 * 1024 * 1024).unwrap();
    let identity = verified.recheck(16 * 1024 * 1024).unwrap();
    assert_eq!(identity.length, original.len() as u64);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(redb::Database::open(&path).is_err());
    retained.write_all_at(&[original[0] ^ 0xff], 0).unwrap();
    assert!(verified.recheck(16 * 1024 * 1024).is_err());
    retained.write_all_at(&original[..1], 0).unwrap();
    assert!(verified.recheck(16 * 1024 * 1024).is_ok());
    std::fs::remove_file(&path).unwrap();
    assert!(verified.recheck(16 * 1024 * 1024).is_err());
}

#[test]
fn created_backup_matches_locked_source_and_detects_replacement() {
    use crate::redb::backend::CreatedV3Backup;
    use std::os::unix::fs::symlink;
    let (path, db) = database();
    drop(db);
    let target = path.with_extension("created-backup");
    let directory = std::fs::File::open(path.parent().unwrap()).unwrap();
    let source = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let created = CreatedV3Backup::create(
        source,
        directory,
        target.file_name().unwrap(),
        16 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), before);
    assert!(redb::Database::open(&path).is_err());
    assert!(created.recheck(16 * 1024 * 1024).is_ok());
    std::fs::remove_file(&target).unwrap();
    symlink(&path, &target).unwrap();
    assert!(created.recheck(16 * 1024 * 1024).is_err());
    std::fs::remove_file(&target).unwrap();
    drop(created);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn backup_creation_refuses_existing_symlink_and_preserves_source() {
    use crate::redb::backend::CreatedV3Backup;
    use std::os::unix::fs::symlink;
    let (path, db) = database();
    drop(db);
    let target = path.with_extension("existing-backup");
    let before = std::fs::read(&path).unwrap();
    symlink(&path, &target).unwrap();
    let source = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let directory = std::fs::File::open(path.parent().unwrap()).unwrap();
    assert!(
        CreatedV3Backup::create(
            source,
            directory,
            target.file_name().unwrap(),
            16 * 1024 * 1024
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(target).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn verified_backup_rejects_corruption_and_bounds_without_changing_file() {
    use crate::redb::backend::VerifiedV3Backup;
    let (path, db) = database();
    drop(db);
    let bytes = std::fs::read(&path).unwrap();
    assert!(VerifiedV3Backup::inspect(std::fs::File::open(&path).unwrap(), 1).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&path, b"not a redb backup").unwrap();
    let bad = std::fs::read(&path).unwrap();
    assert!(VerifiedV3Backup::inspect(std::fs::File::open(&path).unwrap(), 1024).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bad);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn recovery_inspection_matches_pending_to_the_held_backup_without_writes() {
    use crate::redb::backend::VerifiedV3Backup;
    let (path, db) = database();
    drop(db);
    let backup_path = path.with_extension("backup");
    std::fs::copy(&path, &backup_path).unwrap();
    let backup =
        VerifiedV3Backup::inspect(std::fs::File::open(&backup_path).unwrap(), 16 * 1024 * 1024)
            .unwrap();
    let identity = backup.recheck(16 * 1024 * 1024).unwrap();
    let db = redb::Database::open(&path).unwrap();
    let journal = journal::Journal {
        id: [9; 16],
        backup_length: identity.length,
        backup_digest: identity.digest,
        backup_device: identity.device,
        backup_inode: identity.inode,
        stage: journal::Stage::Pending,
    };
    publish_pending(&db, admission::pending_fixture(&db, journal)).unwrap();
    assert_eq!(
        inspect_recovery_backup(&db, &backup, 16 * 1024 * 1024),
        Ok(journal::Stage::Pending)
    );
    // The extraneous table cannot be the result of atomic Pending.
    let unexpected = TableDefinition::<&[u8], &[u8]>::new("unexpected");
    let write = db.begin_write().unwrap();
    drop(write.open_table(unexpected).unwrap());
    write.commit().unwrap();
    assert!(inspect_recovery_backup(&db, &backup, 16 * 1024 * 1024).is_err());
    let write = db.begin_write().unwrap();
    assert!(write.delete_table(unexpected).unwrap());
    write.commit().unwrap();
    let other = path.with_extension("other");
    std::fs::copy(&backup_path, &other).unwrap();
    let different =
        VerifiedV3Backup::inspect(std::fs::File::open(&other).unwrap(), 16 * 1024 * 1024).unwrap();
    assert!(inspect_recovery_backup(&db, &different, 16 * 1024 * 1024).is_err());
    assert_eq!(
        inspect_recovery_backup(&db, &backup, 16 * 1024 * 1024),
        Ok(journal::Stage::Pending)
    );
    stage_pending(&db, admission::stage_fixture(&db, [9; 16])).unwrap();
    assert!(matches!(
        inspect_recovery_backup(&db, &backup, 16 * 1024 * 1024),
        Ok(journal::Stage::Staged(_))
    ));
    assert!(inspect_recovery_backup(&db, &different, 16 * 1024 * 1024).is_err());
    drop(db);
    drop(backup);
    drop(different);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(backup_path).unwrap();
    std::fs::remove_file(other).unwrap();
}

#[test]
fn pending_stages_and_switches_only_under_matching_one_shot_permits() {
    use carosign_core::ids::{IdempotencyKey, OperationId};
    let (path, db) = database();
    let id = [7; 16];
    let record = crate::model::IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: None,
    };
    let key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    let value = crate::mutation_records::encode_idempotency(&record).unwrap();
    let pending = journal::Journal {
        id,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Pending,
    };
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(TableDefinition::<&[u8], &[u8]>::new("idempotency"))
            .unwrap()
            .insert(key.as_slice(), value.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        publish_pending(&db, admission::pending_fixture(&db, pending)),
        Ok(())
    );
    assert_eq!(
        publish_pending(&db, admission::pending_fixture(&db, pending)),
        Err(SwitchFailure::Invalid)
    );
    assert_eq!(
        stage_pending(&db, admission::stage_fixture(&db, [8; 16])),
        Err(SwitchFailure::Invalid)
    );
    assert_eq!(
        stage_pending_inner(&db, admission::stage_fixture(&db, id), |_| {
            Err(SwitchFailure::CommitUnknown)
        }),
        Err(SwitchFailure::CommitUnknown)
    );
    let read = db.begin_read().unwrap();
    let meta = read
        .open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
        .unwrap();
    assert_eq!(
        journal::Journal::decode(meta.get(JOURNAL_KEY).unwrap().unwrap().value())
            .unwrap()
            .stage,
        journal::Stage::Pending
    );
    drop(meta);
    drop(read);
    assert_eq!(
        stage_pending(&db, admission::stage_fixture(&db, id)),
        Ok(())
    );
    assert_eq!(audit_staging_snapshot(&db.begin_read().unwrap()), Ok(()));
    assert_eq!(
        stage_pending(&db, admission::stage_fixture(&db, id)),
        Err(SwitchFailure::Invalid)
    );
    assert_eq!(switch_staged(&db, admission::fixture(&db, id)), Ok(()));
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn lost_staging_commit_ack_does_not_permit_blind_repeat() {
    let (path, db) = database();
    let id = [9; 16];
    let pending = journal::Journal {
        id,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Pending,
    }
    .encode()
    .unwrap();
    let tx = db.begin_write().unwrap();
    tx.open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
        .unwrap()
        .insert(JOURNAL_KEY, pending.as_slice())
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(
        stage_pending_inner(&db, admission::stage_fixture(&db, id), |tx| {
            tx.commit().unwrap();
            Err(SwitchFailure::CommitUnknown)
        }),
        Err(SwitchFailure::CommitUnknown)
    );
    assert_eq!(audit_staging_snapshot(&db.begin_read().unwrap()), Ok(()));
    assert_eq!(
        stage_pending(&db, admission::stage_fixture(&db, id)),
        Err(SwitchFailure::Invalid)
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unsupported_source_never_leaves_partial_staging_or_advances_pending() {
    let (path, db) = database();
    let id = [10; 16];
    let pending = journal::Journal {
        id,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Pending,
    };
    assert_eq!(
        publish_pending(&db, admission::pending_fixture(&db, pending)),
        Ok(())
    );
    let tx = db.begin_write().unwrap();
    tx.open_table(TableDefinition::<&[u8], &[u8]>::new("provider_state"))
        .unwrap()
        .insert(&[1u8; 32][..], &[5u8][..])
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(
        stage_pending(&db, admission::stage_fixture(&db, id)),
        Err(SwitchFailure::Invalid)
    );
    let read = db.begin_read().unwrap();
    let meta = read
        .open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
        .unwrap();
    assert_eq!(
        journal::Journal::decode(meta.get(JOURNAL_KEY).unwrap().unwrap().value())
            .unwrap()
            .stage,
        journal::Stage::Pending
    );
    assert!(
        read.open_table(TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE))
            .is_err()
    );
    drop(meta);
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn lost_pending_commit_ack_refuses_republication_and_requires_inspection() {
    let (path, db) = database();
    let journal = journal::Journal {
        id: [11; 16],
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Pending,
    };
    assert_eq!(
        publish_pending_inner(&db, admission::pending_fixture(&db, journal), |tx| {
            tx.commit().unwrap();
            Err(SwitchFailure::CommitUnknown)
        }),
        Err(SwitchFailure::CommitUnknown)
    );
    assert_eq!(
        publish_pending(&db, admission::pending_fixture(&db, journal)),
        Err(SwitchFailure::Invalid)
    );
    let read = db.begin_read().unwrap();
    let meta = read
        .open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
        .unwrap();
    assert_eq!(
        journal::Journal::decode(meta.get(JOURNAL_KEY).unwrap().unwrap().value()).unwrap(),
        journal
    );
    drop(meta);
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn guarded_switch_commits_all_records_marker_and_journal_together() {
    use carosign_core::ids::{IdempotencyKey, OperationId};
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind, Value};

    let (path, db) = database();
    let id = [5; 16];
    let record = crate::model::IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: None,
    };
    let source_key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    let original = crate::mutation_records::encode_idempotency(&record).unwrap();
    let target = crate::installation_records::prepare_v3_record(
        MigratableRecord::Idempotency,
        &source_key,
        &original,
    )
    .unwrap();
    let key = Key::encode(id, TableKind::Idempotency, &source_key).unwrap();
    let value = Value::encode(
        TableKind::Idempotency,
        crate::mutation_plan::sha256(&original),
        &target,
    )
    .unwrap();
    let mut hash = Sha256::new();
    for bytes in [key.as_slice(), value.as_slice()] {
        hash.update((bytes.len() as u32).to_be_bytes());
        hash.update(bytes);
    }
    let journal = journal::Journal {
        id,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Staged(journal::Snapshot {
            count: 1,
            digest: hash.finalize().into(),
        }),
    }
    .encode()
    .unwrap();
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let source_def = TableDefinition::<&[u8], &[u8]>::new("idempotency");
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(meta_def)
            .unwrap()
            .insert(JOURNAL_KEY, journal.as_slice())
            .unwrap();
        tx.open_table(source_def)
            .unwrap()
            .insert(source_key.as_slice(), original.as_slice())
            .unwrap();
        tx.open_table(stage_def)
            .unwrap()
            .insert(key.as_slice(), value.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    let old = db.begin_read().unwrap();
    let wrong = admission::fixture(&db, [6; 16]);
    assert_eq!(switch_staged(&db, wrong), Err(SwitchFailure::Invalid));
    assert_eq!(audit_staging_snapshot(&db.begin_read().unwrap()), Ok(()));
    // The staging violation must interrupt the switch to commit and not change either the
    // source record or the schema marker.
    let tx = db.begin_write().unwrap();
    {
        let mut damaged = value.clone();
        *damaged.last_mut().unwrap() ^= 1;
        tx.open_table(stage_def)
            .unwrap()
            .insert(key.as_slice(), damaged.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        switch_staged_inner(&db, admission::fixture(&db, id), |_| {
            panic!("повреждённый staging нельзя фиксировать")
        }),
        Err(SwitchFailure::Invalid)
    );
    {
        let current = db.begin_read().unwrap();
        assert_eq!(
            current
                .open_table(meta_def)
                .unwrap()
                .get(crate::redb::backend::SCHEMA_KEY)
                .unwrap()
                .unwrap()
                .value(),
            crate::redb::backend::SCHEMA_V3
        );
        assert_eq!(
            current
                .open_table(source_def)
                .unwrap()
                .get(source_key.as_slice())
                .unwrap()
                .unwrap()
                .value(),
            original
        );
    }
    let tx = db.begin_write().unwrap();
    tx.open_table(stage_def)
        .unwrap()
        .insert(key.as_slice(), value.as_slice())
        .unwrap();
    tx.commit().unwrap();
    let permit = admission::fixture(&db, id);
    // Loss of confirmation after actual commit does not allow repeating the switch: the new
    // phase is read from the database.
    assert_eq!(
        switch_staged_inner(&db, permit, |write| {
            write.commit().unwrap();
            Err(SwitchFailure::CommitUnknown)
        }),
        Err(SwitchFailure::CommitUnknown)
    );
    let current = db.begin_read().unwrap();
    let meta = current.open_table(meta_def).unwrap();
    assert_eq!(
        meta.get(crate::redb::backend::SCHEMA_KEY)
            .unwrap()
            .unwrap()
            .value(),
        b"BSC1\0\0\0\x04\0\0\0\0"
    );
    assert!(matches!(
        journal::Journal::decode(meta.get(JOURNAL_KEY).unwrap().unwrap().value())
            .unwrap()
            .stage,
        journal::Stage::Committed(_)
    ));
    assert_eq!(
        current
            .open_table(source_def)
            .unwrap()
            .get(source_key.as_slice())
            .unwrap()
            .unwrap()
            .value(),
        target
    );
    assert_eq!(
        old.open_table(source_def)
            .unwrap()
            .get(source_key.as_slice())
            .unwrap()
            .unwrap()
            .value(),
        original
    );
    assert!(
        current
            .open_table(TableDefinition::<&[u8], &[u8]>::new(
                "provider_installations"
            ))
            .is_ok()
    );
    assert_eq!(
        switch_staged(&db, admission::fixture(&db, id)),
        Err(SwitchFailure::Invalid)
    );
    drop(meta);
    drop(current);
    drop(old);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn guarded_switch_without_commit_preserves_v3_and_allows_fresh_audit() {
    let (path, db) = database();
    let id = [7; 16];
    let entry = journal::Journal {
        id,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Staged(journal::Snapshot {
            count: 0,
            digest: crate::mutation_plan::sha256(&[]),
        }),
    }
    .encode()
    .unwrap();
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(meta_def)
            .unwrap()
            .insert(JOURNAL_KEY, entry.as_slice())
            .unwrap();
        drop(tx.open_table(stage_def).unwrap());
    }
    tx.commit().unwrap();
    let permit = admission::fixture(&db, id);
    assert_eq!(
        switch_staged_inner(&db, permit, |write| {
            drop(write);
            Err(SwitchFailure::CommitUnknown)
        }),
        Err(SwitchFailure::CommitUnknown)
    );
    assert_eq!(audit_staging_snapshot(&db.begin_read().unwrap()), Ok(()));
    // This is a repeat only for the pure transaction test: the production owner, with an
    // unknown commit, must first establish a stable phase.
    assert_eq!(switch_staged(&db, admission::fixture(&db, id)), Ok(()));
    assert_eq!(
        switch_staged(&db, admission::fixture(&db, id)),
        Err(SwitchFailure::Invalid)
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn staged_enrollment_requires_matching_unchanged_index() {
    use carosign_core::{
        enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef},
        identity::SubjectRef,
        ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId},
    };
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind, Value};
    let (path, db) = database();
    let migration = [5; 16];
    let record = crate::enrollment_codec::EnrollmentRecord {
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([1; 16]).unwrap(),
            subject: SubjectId::from_bytes([2; 16]).unwrap(),
        },
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([3; 16]).unwrap(),
            authenticator: AuthenticatorId::from_bytes([4; 16]).unwrap(),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "образец".into(),
        status: crate::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![7]).unwrap(),
        format_id: "test".into(),
        format_version: 1,
        binding_kind: crate::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let source = crate::enrollment_codec::encode_v3(&record).unwrap();
    let enrollment_key = record.enrollment.id.as_bytes();
    let index_key = crate::redb::codec::enrollment_index_key(&record);
    let prepared = crate::installation_records::prepare_v3_record(
        MigratableRecord::Enrollment,
        enrollment_key,
        &source,
    )
    .unwrap();
    let mut rows = std::collections::BTreeMap::new();
    for (kind, key, before, after) in [
        (
            TableKind::Enrollments,
            enrollment_key.as_slice(),
            source.as_slice(),
            prepared.as_slice(),
        ),
        (
            TableKind::EnrollmentIndex,
            index_key.as_slice(),
            enrollment_key.as_slice(),
            enrollment_key.as_slice(),
        ),
    ] {
        rows.insert(
            Key::encode(migration, kind, key).unwrap(),
            Value::encode(kind, crate::mutation_plan::sha256(before), after).unwrap(),
        );
    }
    let mut digest = Sha256::new();
    for (key, value) in &rows {
        digest.update((key.len() as u32).to_be_bytes());
        digest.update(key);
        digest.update((value.len() as u32).to_be_bytes());
        digest.update(value);
    }
    let journal = journal::Journal {
        id: migration,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Staged(journal::Snapshot {
            count: 2,
            digest: digest.finalize().into(),
        }),
    }
    .encode()
    .unwrap();
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
            .unwrap()
            .insert(JOURNAL_KEY, journal.as_slice())
            .unwrap();
        tx.open_table(TableDefinition::<&[u8], &[u8]>::new("enrollments"))
            .unwrap()
            .insert(enrollment_key.as_slice(), source.as_slice())
            .unwrap();
        tx.open_table(TableDefinition::<&[u8], &[u8]>::new("enrollment_index"))
            .unwrap()
            .insert(index_key.as_slice(), enrollment_key.as_slice())
            .unwrap();
        let mut stage = tx
            .open_table(TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE))
            .unwrap();
        for (key, value) in &rows {
            stage.insert(key.as_slice(), value.as_slice()).unwrap();
        }
    }
    tx.commit().unwrap();
    assert_eq!(audit_staging_snapshot(&db.begin_read().unwrap()), Ok(()));
    let tx = db.begin_write().unwrap();
    tx.open_table(TableDefinition::<&[u8], &[u8]>::new("enrollment_index"))
        .unwrap()
        .insert(index_key.as_slice(), &[9; 16][..])
        .unwrap();
    tx.commit().unwrap();
    assert!(audit_staging_snapshot(&db.begin_read().unwrap()).is_err());
    drop(db);
    let _ = std::fs::remove_file(path);
}

#[test]
fn cleanup_committed_preserves_journal_and_refuses_corrupt_stage() {
    use redb::ReadableTableMetadata;
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind, Value};
    let (path, db) = database();
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let key = Key::encode([5; 16], TableKind::Idempotency, &[3; 48]).unwrap();
    let value = Value::encode(TableKind::Idempotency, [7; 32], b"prepared").unwrap();
    let mut digest = Sha256::new();
    for bytes in [key.as_slice(), value.as_slice()] {
        digest.update((bytes.len() as u32).to_be_bytes());
        digest.update(bytes);
    }
    let journal = journal::Journal {
        id: [5; 16],
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Committed(journal::Snapshot {
            count: 1,
            digest: digest.finalize().into(),
        }),
    }
    .encode()
    .unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut meta = tx.open_table(meta_def).unwrap();
        meta.insert(
            crate::redb::backend::SCHEMA_KEY,
            &b"BSC1\0\0\0\x04\0\0\0\0"[..],
        )
        .unwrap();
        meta.insert(JOURNAL_KEY, journal.as_slice()).unwrap();
        tx.open_table(stage_def)
            .unwrap()
            .insert(key.as_slice(), value.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    let old = db.begin_read().unwrap();
    let mut unfinished = journal::Journal::decode(&journal).unwrap();
    let journal::Stage::Committed(snapshot) = unfinished.stage else {
        unreachable!();
    };
    unfinished.stage = journal::Stage::Staged(snapshot);
    let tx = db.begin_write().unwrap();
    tx.open_table(meta_def)
        .unwrap()
        .insert(JOURNAL_KEY, unfinished.encode().unwrap().as_slice())
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(cleanup_committed_staging(&db), Err(CleanupFailure::Invalid));
    let tx = db.begin_write().unwrap();
    tx.open_table(meta_def)
        .unwrap()
        .insert(JOURNAL_KEY, journal.as_slice())
        .unwrap();
    tx.commit().unwrap();
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(stage_def).unwrap();
        let mut corrupt = value.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        table.insert(key.as_slice(), corrupt.as_slice()).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(cleanup_committed_staging(&db), Err(CleanupFailure::Invalid));
    let read = db.begin_read().unwrap();
    assert_eq!(read.open_table(stage_def).unwrap().len().unwrap(), 1);
    drop(read);
    let tx = db.begin_write().unwrap();
    tx.open_table(stage_def)
        .unwrap()
        .insert(key.as_slice(), value.as_slice())
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(cleanup_committed_staging(&db), Ok(true));
    assert_eq!(cleanup_committed_staging(&db), Ok(false));
    let read = db.begin_read().unwrap();
    assert_eq!(
        read.open_table(meta_def)
            .unwrap()
            .get(JOURNAL_KEY)
            .unwrap()
            .unwrap()
            .value(),
        journal
    );
    assert!(read.open_table(stage_def).is_err());
    assert_eq!(old.open_table(stage_def).unwrap().len().unwrap(), 1);
    drop(read);
    drop(old);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn staging_audit_rejects_package_identity_collision_with_matching_hashes() {
    use crate::installation_records::{PackageRecord, RegisteredProfile};
    use prost::Message;
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind, Value};
    let (path, db) = database();
    let migration = [5; 16];
    let mut entries = Vec::new();
    for name in ["org.example.first", "org.example.second"] {
        let package = PackageRecord {
            package_name: name.into(),
            package_id: vec![1; 16],
            provider_id: vec![2; 16],
            installed_version: "1.0".into(),
            approved_profiles: vec![RegisteredProfile {
                profile_name: "default".into(),
                profile_id: vec![3; 16],
            }],
        };
        let mut payload = vec![0x08, 0x03, 0x82, 0x01];
        package.encode_length_delimited(&mut payload).unwrap();
        let original =
            crate::enrollment_codec::envelope_with(&payload, crate::mutation_plan::sha256).unwrap();
        let prepared =
            crate::installation_records::prepare_v3_package(name.as_bytes(), &original).unwrap();
        let key = Key::encode(migration, TableKind::Packages, name.as_bytes()).unwrap();
        let value = Value::encode(
            TableKind::Packages,
            crate::mutation_plan::sha256(&original),
            &prepared,
        )
        .unwrap();
        entries.push((name, original, key, value));
    }
    entries.sort_by(|a, b| a.2.cmp(&b.2));
    let mut digest = Sha256::new();
    for (_, _, key, value) in &entries {
        for bytes in [key.as_slice(), value.as_slice()] {
            digest.update((bytes.len() as u32).to_be_bytes());
            digest.update(bytes);
        }
    }
    let journal = journal::Journal {
        id: migration,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Staged(journal::Snapshot {
            count: 2,
            digest: digest.finalize().into(),
        }),
    }
    .encode()
    .unwrap();
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(TableDefinition::<&[u8], &[u8]>::new("meta"))
            .unwrap()
            .insert(JOURNAL_KEY, journal.as_slice())
            .unwrap();
        let mut packages = tx
            .open_table(TableDefinition::<&[u8], &[u8]>::new("packages"))
            .unwrap();
        let mut staging = tx
            .open_table(TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE))
            .unwrap();
        for (name, original, key, value) in &entries {
            packages
                .insert(name.as_bytes(), original.as_slice())
                .unwrap();
            staging.insert(key.as_slice(), value.as_slice()).unwrap();
        }
    }
    tx.commit().unwrap();
    assert_eq!(
        audit_staging_snapshot(&db.begin_read().unwrap()),
        Err(PreflightFailure::Corrupt)
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn staging_audit_uses_one_snapshot_and_rejects_missing_extra_and_changed_source() {
    use carosign_core::ids::{IdempotencyKey, OperationId};
    use sha2::{Digest, Sha256};
    use staging::{Key, TableKind, Value};
    let (path, db) = database();
    let migration = [5; 16];
    let record = crate::model::IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: None,
    };
    let source_key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    let source = crate::mutation_records::encode_idempotency(&record).unwrap();
    let prepared = crate::installation_records::prepare_v3_record(
        MigratableRecord::Idempotency,
        &source_key,
        &source,
    )
    .unwrap();
    let key = Key::encode(migration, TableKind::Idempotency, &source_key).unwrap();
    let value = Value::encode(
        TableKind::Idempotency,
        crate::mutation_plan::sha256(&source),
        &prepared,
    )
    .unwrap();
    let mut digest = Sha256::new();
    digest.update((key.len() as u32).to_be_bytes());
    digest.update(&key);
    digest.update((value.len() as u32).to_be_bytes());
    digest.update(&value);
    let journal = journal::Journal {
        id: migration,
        backup_length: 10,
        backup_digest: [2; 32],
        backup_device: 0,
        backup_inode: 1,
        stage: journal::Stage::Staged(journal::Snapshot {
            count: 1,
            digest: digest.finalize().into(),
        }),
    }
    .encode()
    .unwrap();
    let stage_def = TableDefinition::<&[u8], &[u8]>::new(STAGING_TABLE);
    let meta_def = TableDefinition::<&[u8], &[u8]>::new("meta");
    let source_def = TableDefinition::<&[u8], &[u8]>::new("idempotency");
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(meta_def)
            .unwrap()
            .insert(JOURNAL_KEY, journal.as_slice())
            .unwrap();
        tx.open_table(source_def)
            .unwrap()
            .insert(source_key.as_slice(), source.as_slice())
            .unwrap();
        tx.open_table(stage_def)
            .unwrap()
            .insert(key.as_slice(), value.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    let old = db.begin_read().unwrap();
    assert_eq!(audit_staging_snapshot(&old), Ok(()));
    // All staging-bytes and their digest can be correct, but the reference of idempotence to
    // a non-existent change is still unacceptable.
    let mut dangling = record.clone();
    dangling.mutation = Some(carosign_core::ids::MutationId::from_bytes([9; 16]).unwrap());
    let dangling_source = crate::mutation_records::encode_idempotency(&dangling).unwrap();
    let dangling_prepared = crate::installation_records::prepare_v3_record(
        MigratableRecord::Idempotency,
        &source_key,
        &dangling_source,
    )
    .unwrap();
    let dangling_value = Value::encode(
        TableKind::Idempotency,
        crate::mutation_plan::sha256(&dangling_source),
        &dangling_prepared,
    )
    .unwrap();
    let mut dangling_digest = Sha256::new();
    for bytes in [key.as_slice(), dangling_value.as_slice()] {
        dangling_digest.update((bytes.len() as u32).to_be_bytes());
        dangling_digest.update(bytes);
    }
    let mut dangling_journal = journal::Journal::decode(&journal).unwrap();
    dangling_journal.stage = journal::Stage::Staged(journal::Snapshot {
        count: 1,
        digest: dangling_digest.finalize().into(),
    });
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(source_def)
            .unwrap()
            .insert(source_key.as_slice(), dangling_source.as_slice())
            .unwrap();
        tx.open_table(stage_def)
            .unwrap()
            .insert(key.as_slice(), dangling_value.as_slice())
            .unwrap();
        tx.open_table(meta_def)
            .unwrap()
            .insert(JOURNAL_KEY, dangling_journal.encode().unwrap().as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        audit_staging_snapshot(&db.begin_read().unwrap()),
        Err(PreflightFailure::Corrupt)
    );
    assert_eq!(audit_staging_snapshot(&old), Ok(()));
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(source_def)
            .unwrap()
            .insert(source_key.as_slice(), source.as_slice())
            .unwrap();
        tx.open_table(stage_def)
            .unwrap()
            .insert(key.as_slice(), value.as_slice())
            .unwrap();
        tx.open_table(meta_def)
            .unwrap()
            .insert(JOURNAL_KEY, journal.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(source_def)
            .unwrap()
            .remove(source_key.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(audit_staging_snapshot(&old), Ok(()));
    assert!(audit_staging_snapshot(&db.begin_read().unwrap()).is_err());
    drop(old);
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(source_def)
            .unwrap()
            .insert(source_key.as_slice(), source.as_slice())
            .unwrap();
        let mut missing_key = source_key.to_vec();
        missing_key[0] ^= 1;
        tx.open_table(source_def)
            .unwrap()
            .insert(missing_key.as_slice(), source.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    assert!(audit_staging_snapshot(&db.begin_read().unwrap()).is_err());
    let tx = db.begin_write().unwrap();
    {
        let mut missing_key = source_key.to_vec();
        missing_key[0] ^= 1;
        tx.open_table(source_def)
            .unwrap()
            .remove(missing_key.as_slice())
            .unwrap();
        let mut table = tx.open_table(stage_def).unwrap();
        let mut altered = value.clone();
        *altered.last_mut().unwrap() ^= 1;
        table.insert(key.as_slice(), altered.as_slice()).unwrap();
    }
    tx.commit().unwrap();
    assert!(audit_staging_snapshot(&db.begin_read().unwrap()).is_err());
    let tx = db.begin_read().unwrap();
    assert_eq!(
        tx.open_table(source_def)
            .unwrap()
            .get(source_key.as_slice())
            .unwrap()
            .unwrap()
            .value(),
        source
    );
    drop(tx);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn preflight_uses_one_snapshot_and_rejects_unhandled_records() {
    let (path, db) = database();
    let old = db.begin_read().unwrap();
    {
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx
                .open_table(TableDefinition::<&[u8], &[u8]>::new("provider_state"))
                .unwrap();
            table.insert(&b"device"[..], &b"opaque"[..]).unwrap();
        }
        tx.commit().unwrap();
    }
    assert_eq!(
        preflight_v3_snapshot(old),
        Ok(PreflightSummary {
            packages: 0,
            records: 0
        })
    );
    assert_eq!(
        preflight_v3_snapshot(db.begin_read().unwrap()),
        Err(PreflightFailure::UnsupportedRecords)
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn preflight_refuses_unknown_table_and_preserves_its_data() {
    let (path, db) = database();
    let definition = TableDefinition::<&[u8], &[u8]>::new("unexpected");
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(definition).unwrap();
        table.insert(&b"key"[..], &b"original"[..]).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        preflight_v3_snapshot(db.begin_read().unwrap()),
        Err(PreflightFailure::UnsupportedRecords)
    );
    let tx = db.begin_read().unwrap();
    let table = tx.open_table(definition).unwrap();
    assert_eq!(
        table.get(&b"key"[..]).unwrap().unwrap().value(),
        b"original"
    );
    drop(table);
    drop(tx);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn preflight_rejects_damaged_idempotency_without_rewriting_it() {
    let path = std::env::temp_dir().join(format!(
        "carosign-migration-corrupt-{}-{}.redb",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let store = RedbStore::create_v3(&path).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Idempotency, b"invalid-key", b"invalid-value")
        .unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    let db = redb::Database::open(&path).unwrap();
    assert_eq!(
        preflight_v3_snapshot(db.begin_read().unwrap()),
        Err(PreflightFailure::Corrupt)
    );
    let tx = db.begin_read().unwrap();
    let table = tx
        .open_table(TableDefinition::<&[u8], &[u8]>::new("idempotency"))
        .unwrap();
    assert_eq!(
        table.get(&b"invalid-key"[..]).unwrap().unwrap().value(),
        b"invalid-value"
    );
    drop(table);
    drop(tx);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn preflight_refuses_consistent_prepared_mutation() {
    use crate::model::{IdempotencyRecord, MutationPhase, MutationRecord};
    use crate::mutation_plan::{
        BindingSnapshot, MutationPlan, Scope, StoredBindingKind, Transition,
    };
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{
        AuthenticatorId, AuthorityId, EnrollmentId, IdempotencyKey, MutationId, OperationId,
        SubjectId,
    };

    let path = std::env::temp_dir().join(format!(
        "carosign-migration-active-{}-{}.redb",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let store = RedbStore::create_v3(&path).unwrap();
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
    let plan = MutationPlan::build(
        Transition::Remove,
        Some(target),
        None,
        0,
        vec![BindingSnapshot {
            enrollment: target,
            subject,
            kind: StoredBindingKind::ProviderOpaque,
            provider_ref_digest: [5; 32],
            format_id: "example".into(),
            format_version: 1,
        }],
        Scope {
            subject,
            authenticator,
        },
    )
    .unwrap();
    let record = MutationRecord {
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
    };
    let idempotency = IdempotencyRecord {
        caller_digest: [21; 32],
        key: IdempotencyKey::from_bytes([22; 16]).unwrap(),
        request_digest: [23; 32],
        operation: record.operation,
        mutation: Some(record.mutation),
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &crate::mutation_records::encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &record.scope_key(),
        record.mutation.as_bytes(),
    )
    .unwrap();
    let key = crate::redb::codec::idempotency_key(&idempotency.caller_digest, &idempotency.key);
    tx.put(
        Table::Idempotency,
        &key,
        &crate::mutation_records::encode_idempotency(&idempotency).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    drop(store);
    let db = redb::Database::open(&path).unwrap();
    assert_eq!(
        preflight_v3_snapshot(db.begin_read().unwrap()),
        Err(PreflightFailure::ActiveMutation)
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}
