use super::*;
use redb::ReadableTable;
fn sample() -> ProviderInstallationRecord {
    ProviderInstallationRecord {
        package_id: vec![1; 16],
        provider_id: vec![2; 16],
        profile_id: vec![3; 16],
        package_name: "org.example.provider".into(),
        version: "1.0".into(),
        profile_name: "default".into(),
        version_dir: "versions/one".into(),
        device: 1,
        inode: 2,
        signer_key_version: 4,
        signer_fingerprint: vec![9; 20],
        key_policy_id: vec![4; 16],
        revision: 1,
        state: 1,
        policy_path: "policy".into(),
        manifest_path: "manifest".into(),
        profile_path: "profile".into(),
        executable_path: "worker".into(),
        files: ["policy", "manifest", "profile", "worker"]
            .iter()
            .enumerate()
            .map(|(i, p)| InstallationFile {
                path: (*p).into(),
                role: (i + 1) as i32,
                verity_sha256: vec![8; 32],
            })
            .collect(),
    }
}
#[test]
fn roundtrip_and_reject_corruption() {
    let key = [vec![1; 16], vec![3; 16]].concat();
    let record = sample();
    let bytes = encode(&key, &record).unwrap();
    assert_eq!(decode(&key, &bytes).unwrap(), record);
    let mut bad = bytes.clone();
    bad[10] ^= 1;
    assert!(decode(&key, &bad).is_err());
    assert!(decode(&[0; 32], &bytes).is_err());
    let mut bad = record.clone();
    bad.files[1].path = "../escape".into();
    assert!(encode(&key, &bad).is_err());
    let mut bad = record.clone();
    bad.files[1].role = 1;
    assert!(encode(&key, &bad).is_err());
    let mut bad = record.clone();
    bad.files[1].verity_sha256.pop();
    assert!(encode(&key, &bad).is_err());
    let mut bad = record.clone();
    bad.revision = 0;
    assert!(encode(&key, &bad).is_err());
    let mut bad = record;
    bad.state = 99;
    assert!(encode(&key, &bad).is_err());
}

#[test]
fn rejects_multiple_stored_values_and_unknown_fields_even_with_valid_checksum() {
    let key = [vec![1; 16], vec![3; 16]].concat();
    let encoded = encode(&key, &sample()).unwrap();
    let original = unenvelope_with(&encoded, sha256).unwrap();

    // Empty authenticator is the second version of the same oneof.
    // Checksum recalculated: failure must depend on the structure.
    for prefix in [true, false] {
        let mut payload = original.clone();
        if prefix {
            payload.splice(0..0, [0x52, 0x00]);
        } else {
            payload.extend_from_slice(&[0x52, 0x00]);
        }
        let envelope = envelope_with(&payload, sha256).unwrap();
        assert!(decode(&key, &envelope).is_err());
    }

    // An unknown top-level tag should not be lost in parsing.
    let mut payload = original;
    payload.extend_from_slice(&[0xa0, 0x06, 0x01]);
    let envelope = envelope_with(&payload, sha256).unwrap();
    assert!(decode(&key, &envelope).is_err());
}

#[test]
fn package_and_profile_must_match_the_same_registry_snapshot() {
    let key = [vec![1; 16], vec![3; 16]].concat();
    let mut installation = sample();
    let package = PackageRecord {
        package_name: installation.package_name.clone(),
        package_id: installation.package_id.clone(),
        provider_id: installation.provider_id.clone(),
        installed_version: installation.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: installation.profile_name.clone(),
            profile_id: installation.profile_id.clone(),
        }],
    };
    assert!(check_package(&key, &installation, &package).is_ok());

    let mut altered = package.clone();
    altered.provider_id = vec![8; 16];
    assert_eq!(
        check_package(&key, &installation, &altered),
        Err(PackageMismatch)
    );
    altered = package.clone();
    altered.approved_profiles[0].profile_id = vec![8; 16];
    assert_eq!(
        check_package(&key, &installation, &altered),
        Err(PackageMismatch)
    );
    altered = package.clone();
    altered
        .approved_profiles
        .push(altered.approved_profiles[0].clone());
    assert_eq!(
        check_package(&key, &installation, &altered),
        Err(PackageMismatch)
    );
    altered = package.clone();
    altered.installed_version.clear();
    assert_eq!(
        check_package(&key, &installation, &altered),
        Err(PackageMismatch)
    );
    installation.state = 2;
    assert!(check_package(&key, &installation, &altered).is_ok());
    altered.installed_version = "2.0".into();
    assert_eq!(
        check_package(&key, &installation, &altered),
        Err(PackageMismatch)
    );
}

#[test]
fn package_decoder_rejects_wrong_key_schema_and_ambiguous_profiles() {
    let package = PackageRecord {
        package_name: "org.example.provider".into(),
        package_id: vec![1; 16],
        provider_id: vec![2; 16],
        installed_version: "1.0".into(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: "default".into(),
            profile_id: vec![3; 16],
        }],
    };
    let wrap = |package: PackageRecord, schema| {
        let payload = StoredValue {
            schema_version: schema,
            value: Some(Value::PackageRecord(package)),
        }
        .encode_to_vec();
        envelope_with(&payload, sha256).unwrap()
    };
    let key = package.package_name.as_bytes();
    let valid = wrap(package.clone(), 4);
    assert_eq!(decode_package(key, &valid).unwrap(), package);
    assert!(decode_package(b"org.example.other", &valid).is_err());
    assert!(decode_package(key, &wrap(package.clone(), 3)).is_err());
    let mut invalid = package.clone();
    invalid
        .approved_profiles
        .push(invalid.approved_profiles[0].clone());
    assert!(decode_package(key, &wrap(invalid, 4)).is_err());
    let mut invalid = package.clone();
    invalid.provider_id.fill(0);
    assert!(decode_package(key, &wrap(invalid, 4)).is_err());
    let mut invalid = package.clone();
    invalid.installed_version.clear();
    assert!(decode_package(key, &wrap(invalid, 4)).is_ok());
}

#[test]
fn migration_prepares_enrollment_without_reassigning_its_key() {
    use carosign_core::{
        enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef},
        identity::SubjectRef,
        ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId},
    };
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
    let key = record.enrollment.id.as_bytes();
    let output = prepare_v3_record(MigratableRecord::Enrollment, key, &source).unwrap();
    let original =
        StoredValue::decode(unenvelope_with(&source, sha256).unwrap().as_slice()).unwrap();
    let migrated =
        StoredValue::decode(unenvelope_with(&output, sha256).unwrap().as_slice()).unwrap();
    assert_eq!(migrated.schema_version, 4);
    assert_eq!(migrated.value, original.value);
    assert!(prepare_v3_record(MigratableRecord::Enrollment, &[8; 16], &source).is_err());
}

#[test]
fn guarded_v4_checks_enrollment_and_index_in_both_directions() {
    use carosign_core::{
        enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef},
        identity::SubjectRef,
        ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId},
    };
    let path = std::env::temp_dir().join(format!(
        "carosign-v4-enrollment-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    drop(crate::redb::backend::RedbStore::create_current(&path).unwrap());
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
    let key = record.enrollment.id.as_bytes();
    let index_key = crate::redb::codec::enrollment_index_key(&record);
    let encoded = prepare_v3_record(
        MigratableRecord::Enrollment,
        key,
        &crate::enrollment_codec::encode_v3(&record).unwrap(),
    )
    .unwrap();
    let enrollments = redb::TableDefinition::<&[u8], &[u8]>::new("enrollments");
    let index = redb::TableDefinition::<&[u8], &[u8]>::new("enrollment_index");
    let db = redb::Database::open(&path).unwrap();
    let write = db.begin_write().unwrap();
    write
        .open_table(enrollments)
        .unwrap()
        .insert(key.as_slice(), encoded.as_slice())
        .unwrap();
    write
        .open_table(index)
        .unwrap()
        .insert(index_key.as_slice(), key.as_slice())
        .unwrap();
    write.commit().unwrap();
    drop(db);
    let reopened = crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap();
    let reader = crate::registry::RegistryReader::new(&reopened);
    assert_eq!(
        reader.by_id(record.enrollment.id).unwrap(),
        crate::registry::Lookup::Found(record.clone())
    );
    let scope: [u8; crate::limits::SCOPE_LOCK_KEY_LEN] = index_key
        [..crate::limits::SCOPE_LOCK_KEY_LEN]
        .try_into()
        .unwrap();
    assert_eq!(reader.by_scope(&scope).unwrap(), vec![record.enrollment.id]);
    drop(reopened);
    let mut vault_record = record.clone();
    vault_record.binding_kind = crate::mutation_plan::StoredBindingKind::ScopedVault;
    vault_record.provider_ref = OpaqueProviderRef::new(Vec::new()).unwrap();
    let vault = prepare_v3_record(
        MigratableRecord::Enrollment,
        key,
        &crate::enrollment_codec::encode_v3(&vault_record).unwrap(),
    )
    .unwrap();
    let db = redb::Database::open(&path).unwrap();
    let write = db.begin_write().unwrap();
    write
        .open_table(enrollments)
        .unwrap()
        .insert(key.as_slice(), vault.as_slice())
        .unwrap();
    write.commit().unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let db = redb::Database::open(&path).unwrap();
    let write = db.begin_write().unwrap();
    write
        .open_table(enrollments)
        .unwrap()
        .insert(key.as_slice(), encoded.as_slice())
        .unwrap();
    write
        .open_table(index)
        .unwrap()
        .remove(index_key.as_slice())
        .unwrap();
    write.commit().unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn migration_prepares_validated_idempotency_without_changing_content() {
    use carosign_core::ids::{IdempotencyKey, OperationId};
    let record = crate::model::IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: None,
    };
    let key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    let source = crate::mutation_records::encode_idempotency(&record).unwrap();
    let output = prepare_v3_record(MigratableRecord::Idempotency, &key, &source).unwrap();
    let decoded = |bytes: &[u8]| {
        let payload = unenvelope_with(bytes, sha256).unwrap();
        StoredValue::decode(payload.as_slice()).unwrap()
    };
    let original = decoded(&source);
    let migrated = decoded(&output);
    assert_eq!(migrated.schema_version, 4);
    assert_eq!(migrated.value, original.value);
    assert!(prepare_v3_record(MigratableRecord::Idempotency, &[9; 48], &source).is_err());
    assert!(prepare_v3_record(MigratableRecord::Mutation, &key, &source).is_err());
    assert!(prepare_v3_record(MigratableRecord::Idempotency, &key, &output).is_err());
    let mut damaged = source.clone();
    damaged[10] ^= 1;
    assert!(prepare_v3_record(MigratableRecord::Idempotency, &key, &damaged).is_err());
    let mut payload = unenvelope_with(&source, sha256).unwrap();
    payload.extend_from_slice(&[0xa0, 0x06, 1]);
    let unknown = envelope_with(&payload, sha256).unwrap();
    assert!(prepare_v3_record(MigratableRecord::Idempotency, &key, &unknown).is_err());
}

#[test]
fn migration_prepares_outbox_with_its_exact_storage_key() {
    use carosign_core::ids::OperationId;
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    let record = crate::outbox::EffectOutboxRecord {
        key: EffectKey {
            operation: OperationId::from_bytes([5; 16]).unwrap(),
            transition_index: TransitionIndex::new(1),
            position: EffectPosition::new(0),
        },
        command: None,
        phase: crate::outbox::EffectPhase::Planned,
        mutation: None,
    };
    let key = record.storage_key();
    let source = crate::mutation_records::encode_outbox(&record).unwrap();
    let output = prepare_v3_record(MigratableRecord::Outbox, &key, &source).unwrap();
    let payload = unenvelope_with(&output, sha256).unwrap();
    let value = StoredValue::decode(payload.as_slice()).unwrap();
    assert_eq!(value.schema_version, 4);
    let mut wrong = key;
    wrong[0] ^= 1;
    assert!(prepare_v3_record(MigratableRecord::Outbox, &wrong, &source).is_err());
}

#[test]
fn package_migration_preserves_identity_and_rejects_unrecognised_input() {
    let record = sample();
    let package = PackageRecord {
        package_name: record.package_name,
        package_id: record.package_id,
        provider_id: record.provider_id,
        installed_version: record.version,
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name,
            profile_id: record.profile_id,
        }],
    };
    let key = package.package_name.as_bytes();
    let source = StoredValue {
        schema_version: 3,
        value: Some(Value::PackageRecord(package.clone())),
    }
    .encode_to_vec();
    let wrapped = envelope_with(&source, sha256).unwrap();
    let prepared = prepare_v3_package(key, &wrapped).unwrap();
    assert_eq!(decode_package(key, &prepared).unwrap(), package);
    assert!(prepare_v3_package(key, &prepared).is_err());
    assert!(prepare_v3_package(b"org.example.other", &wrapped).is_err());

    let mut corrupted = wrapped.clone();
    corrupted[10] ^= 1;
    assert!(prepare_v3_package(key, &corrupted).is_err());
    let mut unknown = source.clone();
    unknown.extend_from_slice(&[0xa0, 0x06, 0x01]);
    assert!(prepare_v3_package(key, &envelope_with(&unknown, sha256).unwrap()).is_err());
    let mut ambiguous = source;
    ambiguous.extend_from_slice(&[0x52, 0]);
    assert!(prepare_v3_package(key, &envelope_with(&ambiguous, sha256).unwrap()).is_err());
    let mut invalid = package.clone();
    invalid.provider_id.fill(0);
    let invalid = StoredValue {
        schema_version: 3,
        value: Some(Value::PackageRecord(invalid)),
    }
    .encode_to_vec();
    assert!(prepare_v3_package(key, &envelope_with(&invalid, sha256).unwrap()).is_err());
}

#[test]
fn package_preflight_reads_one_snapshot_and_refuses_installation_table_under_v3() {
    let path = std::env::temp_dir().join(format!(
        "carosign-v3-package-preflight-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let package = PackageRecord {
        package_name: record.package_name,
        package_id: record.package_id,
        provider_id: record.provider_id,
        installed_version: record.version,
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name,
            profile_id: record.profile_id,
        }],
    };
    let value = envelope_with(
        &StoredValue {
            schema_version: 3,
            value: Some(Value::PackageRecord(package.clone())),
        }
        .encode_to_vec(),
        sha256,
    )
    .unwrap();
    let write = db.begin_write().unwrap();
    {
        write
            .open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, crate::redb::backend::SCHEMA_V3)
            .unwrap();
        write
            .open_table(PACKAGES)
            .unwrap()
            .insert(package.package_name.as_bytes(), value.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    let old = db.begin_read().unwrap();
    assert_eq!(preflight_v3_packages(&old), Ok(1));
    let write = db.begin_write().unwrap();
    {
        let _table = write.open_table(INSTALLATIONS).unwrap();
    }
    write.commit().unwrap();
    assert_eq!(preflight_v3_packages(&old), Ok(1));
    assert_eq!(
        preflight_v3_packages(&db.begin_read().unwrap()),
        Err(SnapshotRefusal::Corrupt)
    );
    drop(old);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unknown_meta_blocks_all_v4_installation_operations_without_changes() {
    let path = std::env::temp_dir().join(format!(
        "carosign-v4-unknown-meta-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let key = [record.package_id.clone(), record.profile_id.clone()].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_key = package.package_name.as_bytes();
    let package_value = envelope_with(
        &StoredValue {
            schema_version: 4,
            value: Some(Value::PackageRecord(package.clone())),
        }
        .encode_to_vec(),
        sha256,
    )
    .unwrap();
    let installation_value = encode(&key, &record).unwrap();
    let write = db.begin_write().unwrap();
    {
        let mut meta = write.open_table(META).unwrap();
        meta.insert(SCHEMA_KEY, SCHEMA_V4).unwrap();
        meta.insert(&b"unknown-migration-state"[..], &b"pending"[..])
            .unwrap();
        write
            .open_table(PACKAGES)
            .unwrap()
            .insert(package_key, package_value.as_slice())
            .unwrap();
        write
            .open_table(INSTALLATIONS)
            .unwrap()
            .insert(key.as_slice(), installation_value.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    assert!(matches!(
        read_active_snapshot(&db.begin_read().unwrap(), package_key, &key),
        Err(SnapshotRefusal::UnsupportedSchema)
    ));
    assert_eq!(
        revalidate_snapshot(&db, package_key, &key, &record),
        Err(SnapshotRefusal::UnsupportedSchema)
    );
    let mut updated = record.clone();
    updated.revision += 1;
    let write = db.begin_write().unwrap();
    assert_eq!(
        stage_publication(&write, package_key, &key, &updated, Some(record.revision)),
        Err(PublicationFailure::UnsupportedSchema)
    );
    assert_eq!(
        stage_revocation(&write, package_key, &key, record.revision),
        Err(RevokeFailure::UnsupportedSchema)
    );
    let profile = carosign_core::ids::ProfileId::try_from(record.profile_id.as_slice()).unwrap();
    assert_eq!(
        stage_uninstall(&write, package_key, &[(profile, record.revision)]),
        Err(RevokeFailure::UnsupportedSchema)
    );
    // Even a mistakenly recorded transaction does not change anything.
    write.commit().unwrap();
    let read = db.begin_read().unwrap();
    let packages = read.open_table(PACKAGES).unwrap();
    let installations = read.open_table(INSTALLATIONS).unwrap();
    assert_eq!(
        packages.get(package_key).unwrap().unwrap().value(),
        package_value
    );
    assert_eq!(
        installations.get(key.as_slice()).unwrap().unwrap().value(),
        installation_value
    );
    drop(packages);
    drop(installations);
    drop(read);
    let write = db.begin_write().unwrap();
    write
        .open_table(META)
        .unwrap()
        .remove(&b"unknown-migration-state"[..])
        .unwrap();
    write.commit().unwrap();
    assert_eq!(
        read_active_snapshot(&db.begin_read().unwrap(), package_key, &key).unwrap(),
        record
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn guarded_open_checks_all_installations_and_preserves_invalid_database() {
    let path = std::env::temp_dir().join(format!(
        "carosign-v4-open-installations-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    drop(crate::redb::backend::RedbStore::create_current(&path).unwrap());
    let record = sample();
    let key = [record.package_id.clone(), record.profile_id.clone()].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_value = envelope_with(
        &StoredValue {
            schema_version: 4,
            value: Some(Value::PackageRecord(package.clone())),
        }
        .encode_to_vec(),
        sha256,
    )
    .unwrap();
    let db = redb::Database::open(&path).unwrap();
    let write = db.begin_write().unwrap();
    write
        .open_table(PACKAGES)
        .unwrap()
        .insert(package.package_name.as_bytes(), package_value.as_slice())
        .unwrap();
    write
        .open_table(INSTALLATIONS)
        .unwrap()
        .insert(key.as_slice(), encode(&key, &record).unwrap().as_slice())
        .unwrap();
    write.commit().unwrap();
    drop(db);
    drop(crate::redb::backend::RedbStore::open_current_guarded(&path).unwrap());
    let db = redb::Database::open(&path).unwrap();
    let write = db.begin_write().unwrap();
    write
        .open_table(PACKAGES)
        .unwrap()
        .remove(package.package_name.as_bytes())
        .unwrap();
    write.commit().unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn snapshot_reads_matching_records_and_rejects_v3_and_revoked() {
    use redb::Database;
    let path = std::env::temp_dir().join(format!(
        "carosign-v4-snapshot-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = Database::create(&path).unwrap();
    let record = sample();
    let key = [vec![1; 16], vec![3; 16]].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_key = package.package_name.as_bytes();
    let package_value = envelope_with(
        &StoredValue {
            schema_version: 4,
            value: Some(Value::PackageRecord(package.clone())),
        }
        .encode_to_vec(),
        sha256,
    )
    .unwrap();
    let write = db.begin_write().unwrap();
    {
        let mut meta = write.open_table(META).unwrap();
        meta.insert(SCHEMA_KEY, &b"BSC1\0\0\0\x03\0\0\0\0"[..])
            .unwrap();
        let mut packages = write.open_table(PACKAGES).unwrap();
        packages
            .insert(package_key, package_value.as_slice())
            .unwrap();
        let mut installations = write.open_table(INSTALLATIONS).unwrap();
        let encoded = encode(&key, &record).unwrap();
        installations
            .insert(key.as_slice(), encoded.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    let v3 = db.begin_read().unwrap();
    assert!(matches!(
        read_active_snapshot(&v3, package_key, &key),
        Err(SnapshotRefusal::UnsupportedSchema)
    ));
    drop(v3);

    let write = db.begin_write().unwrap();
    {
        let mut meta = write.open_table(META).unwrap();
        meta.insert(SCHEMA_KEY, SCHEMA_V4).unwrap();
    }
    write.commit().unwrap();
    let before_revoke = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&before_revoke, package_key, &key).unwrap(),
        record
    );
    assert_eq!(revalidate_snapshot(&db, package_key, &key, &record), Ok(()));
    let write = db.begin_write().unwrap();
    {
        let mut installations = write.open_table(INSTALLATIONS).unwrap();
        let mut changed = record.clone();
        changed.files[0].verity_sha256[0] ^= 1;
        let encoded = encode(&key, &changed).unwrap();
        installations
            .insert(key.as_slice(), encoded.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    assert_eq!(
        revalidate_snapshot(&db, package_key, &key, &record),
        Err(SnapshotRefusal::Changed)
    );
    let write = db.begin_write().unwrap();
    {
        let mut installations = write.open_table(INSTALLATIONS).unwrap();
        let mut changed = record.clone();
        changed.revision += 1;
        let encoded = encode(&key, &changed).unwrap();
        installations
            .insert(key.as_slice(), encoded.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    assert_eq!(
        revalidate_snapshot(&db, package_key, &key, &record),
        Err(SnapshotRefusal::Changed)
    );
    let write = db.begin_write().unwrap();
    assert_eq!(
        stage_revocation(&write, package_key, &key, 1),
        Err(RevokeFailure::Conflict)
    );
    write.commit().unwrap();
    let write = db.begin_write().unwrap();
    assert_eq!(stage_revocation(&write, package_key, &key, 2), Ok(()));
    write.abort().unwrap();
    let check = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&check, package_key, &key)
            .unwrap()
            .revision,
        2
    );
    drop(check);
    let write = db.begin_write().unwrap();
    assert_eq!(stage_revocation(&write, package_key, &key, 2), Ok(()));
    write.commit().unwrap();
    assert_eq!(
        read_active_snapshot(&before_revoke, package_key, &key).unwrap(),
        record
    );
    drop(before_revoke);
    let after_revoke = db.begin_read().unwrap();
    assert!(matches!(
        read_active_snapshot(&after_revoke, package_key, &key),
        Err(SnapshotRefusal::Revoked)
    ));
    assert_eq!(
        revalidate_snapshot(&db, package_key, &key, &record),
        Err(SnapshotRefusal::Revoked)
    );
    drop(after_revoke);
    let write = db.begin_write().unwrap();
    assert_eq!(
        stage_revocation(&write, package_key, &key, 2),
        Err(RevokeFailure::Conflict)
    );
    write.abort().unwrap();
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn publication_refuses_reused_package_identifiers() {
    let path = std::env::temp_dir().join(format!(
        "carosign-package-identity-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let other_name = "org.example.other";
    let key = [record.package_id.clone(), record.profile_id.clone()].concat();
    for collision in 0..3 {
        let tx = db.begin_write().unwrap();
        {
            let mut meta = tx.open_table(META).unwrap();
            meta.insert(SCHEMA_KEY, SCHEMA_V4).unwrap();
            let mut packages = tx.open_table(PACKAGES).unwrap();
            let mut other = package.clone();
            other.package_name = other_name.into();
            if collision != 0 {
                other.package_id = vec![11; 16];
            }
            if collision != 1 {
                other.provider_id = vec![12; 16];
            }
            if collision != 2 {
                other.approved_profiles[0].profile_id = vec![13; 16];
            }
            for entry in [&package, &other] {
                let bytes = envelope_with(
                    &StoredValue {
                        schema_version: 4,
                        value: Some(Value::PackageRecord(entry.clone())),
                    }
                    .encode_to_vec(),
                    sha256,
                )
                .unwrap();
                packages
                    .insert(entry.package_name.as_bytes(), bytes.as_slice())
                    .unwrap();
            }
            tx.open_table(INSTALLATIONS).unwrap();
        }
        assert_eq!(
            stage_publication(&tx, package.package_name.as_bytes(), &key, &record, None),
            Err(PublicationFailure::Corrupt)
        );
        // Even the previously damaged base does not allow launch.
        tx.open_table(INSTALLATIONS)
            .unwrap()
            .insert(key.as_slice(), encode(&key, &record).unwrap().as_slice())
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(
            read_active_snapshot(
                &db.begin_read().unwrap(),
                package.package_name.as_bytes(),
                &key
            ),
            Err(SnapshotRefusal::Corrupt)
        );
    }
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn initial_publication_refuses_missing_table_without_creating_it() {
    let path = std::env::temp_dir().join(format!(
        "carosign-publication-missing-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let key = [record.package_id.clone(), record.profile_id.clone()].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let write = db.begin_write().unwrap();
    write
        .open_table(META)
        .unwrap()
        .insert(SCHEMA_KEY, SCHEMA_V4)
        .unwrap();
    write.open_table(PACKAGES).unwrap();
    write.commit().unwrap();
    assert_eq!(
        publish_initial_package_and_installation(&db, &package, &key, &record),
        Err(PublicationFailure::Corrupt)
    );
    let read = db.begin_read().unwrap();
    assert!(read.open_table(INSTALLATIONS).is_err());
    assert!(
        read.open_table(PACKAGES)
            .unwrap()
            .get(package.package_name.as_bytes())
            .unwrap()
            .is_none()
    );
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn policy_change_at_publication_refuses_or_reports_unknown() {
    for failed_check in [1usize, 2, 3] {
        let path = std::env::temp_dir().join(format!(
            "carosign-publication-policy-{}-{:?}-{failed_check}.redb",
            std::process::id(),
            std::thread::current().id()
        ));
        let db = redb::Database::create(&path).unwrap();
        let record = sample();
        let key = [record.package_id.clone(), record.profile_id.clone()].concat();
        let package = PackageRecord {
            package_name: record.package_name.clone(),
            package_id: record.package_id.clone(),
            provider_id: record.provider_id.clone(),
            installed_version: record.version.clone(),
            approved_profiles: vec![RegisteredProfile {
                profile_name: record.profile_name.clone(),
                profile_id: record.profile_id.clone(),
            }],
        };
        let tx = db.begin_write().unwrap();
        tx.open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, SCHEMA_V4)
            .unwrap();
        tx.open_table(PACKAGES).unwrap();
        tx.open_table(INSTALLATIONS).unwrap();
        tx.commit().unwrap();
        let mut checks = 0;
        let outcome = publish_initial_with_provenance(
            &db,
            &package,
            &key,
            &record,
            Some([[7; 32], [8; 32]]),
            || {
                checks += 1;
                checks != failed_check
            },
        );
        assert_eq!(
            outcome,
            Err(if failed_check == 3 {
                PublicationFailure::CommitUnknown
            } else {
                PublicationFailure::PolicyChanged
            })
        );
        let tx = db.begin_read().unwrap();
        let table = tx.open_table(PACKAGES).unwrap();
        assert_eq!(
            table
                .get(package.package_name.as_bytes())
                .unwrap()
                .is_some(),
            failed_check == 3
        );
        drop(table);
        drop(tx);
        drop(db);
        if failed_check == 3 {
            // ACTIVE may have survived, but once restarted, the barrier does not allow
            // recording to be used without a separate recovery.
            let reopened = redb::Database::open(&path).unwrap();
            let read = reopened.begin_read().unwrap();
            assert!(matches!(
                read_active_snapshot(&read, package.package_name.as_bytes(), &key),
                Err(SnapshotRefusal::UnsupportedSchema)
            ));
            let prepared =
                read_quarantined_snapshot(&read, package.package_name.as_bytes(), &key).unwrap();
            assert_eq!(prepared.installation, record);
            assert_eq!(prepared.package, package);
            assert_eq!(
                prepared.provenance,
                Some([[7u8; 32], [8u8; 32]].concat().try_into().unwrap())
            );
            assert!(
                revalidate_quarantined_snapshot(
                    &reopened,
                    package.package_name.as_bytes(),
                    &key,
                    &prepared
                )
                .is_ok()
            );
            let mut stale_provenance = QuarantinedSnapshot {
                package: package.clone(),
                installation: record.clone(),
                provenance: Some([9; 64]),
            };
            assert!(
                revalidate_quarantined_snapshot(
                    &reopened,
                    package.package_name.as_bytes(),
                    &key,
                    &stale_provenance,
                )
                .is_err()
            );
            stale_provenance.provenance = None;
            assert!(
                revalidate_quarantined_snapshot(
                    &reopened,
                    package.package_name.as_bytes(),
                    &key,
                    &stale_provenance,
                )
                .is_err()
            );
            let mut stale = record.clone();
            stale.revision += 1;
            let stale_snapshot = QuarantinedSnapshot {
                package: package.clone(),
                installation: stale.clone(),
                provenance: prepared.provenance,
            };
            assert_eq!(
                revalidate_quarantined_snapshot(
                    &reopened,
                    package.package_name.as_bytes(),
                    &key,
                    &stale_snapshot
                ),
                Err(SnapshotRefusal::Changed)
            );
            assert!(matches!(
                read_quarantined_snapshot(&read, package.package_name.as_bytes(), b"wrong"),
                Err(SnapshotRefusal::UnsupportedSchema)
            ));
            drop(read);
            // The packet can change without changing the installation record: for example,
            // another approved profile has been added to it.
            // Recovery must also detect such a change.
            let mut changed_package = package.clone();
            changed_package.approved_profiles.push(RegisteredProfile {
                profile_name: "other.profile".to_owned(),
                profile_id: vec![0x61; 16],
            });
            let write = reopened.begin_write().unwrap();
            {
                let mut packages = write.open_table(PACKAGES).unwrap();
                let payload = StoredValue {
                    schema_version: 4,
                    value: Some(Value::PackageRecord(changed_package)),
                }
                .encode_to_vec();
                let bytes = envelope_with(&payload, sha256).unwrap();
                packages
                    .insert(package.package_name.as_bytes(), bytes.as_slice())
                    .unwrap();
            }
            write.commit().unwrap();
            assert_eq!(
                revalidate_quarantined_snapshot(
                    &reopened,
                    package.package_name.as_bytes(),
                    &key,
                    &prepared
                ),
                Err(SnapshotRefusal::Changed)
            );
            let write = reopened.begin_write().unwrap();
            {
                let mut packages = write.open_table(PACKAGES).unwrap();
                let payload = StoredValue {
                    schema_version: 4,
                    value: Some(Value::PackageRecord(package.clone())),
                }
                .encode_to_vec();
                let bytes = envelope_with(&payload, sha256).unwrap();
                packages
                    .insert(package.package_name.as_bytes(), bytes.as_slice())
                    .unwrap();
            }
            write.commit().unwrap();
            // Another transaction changes the record after a preliminary shot: a recheck must
            // open a new shot.
            let write = reopened.begin_write().unwrap();
            {
                let mut installations = write.open_table(INSTALLATIONS).unwrap();
                let changed = encode(&key, &stale).unwrap();
                installations
                    .insert(key.as_slice(), changed.as_slice())
                    .unwrap();
            }
            write.commit().unwrap();
            assert_eq!(
                revalidate_quarantined_snapshot(
                    &reopened,
                    package.package_name.as_bytes(),
                    &key,
                    &prepared
                ),
                Err(SnapshotRefusal::Changed)
            );
            // Damage to another packet cannot be concealed by selectively reading the
            // recoverable unit from the same image.
            let write = reopened.begin_write().unwrap();
            {
                let mut packages = write.open_table(PACKAGES).unwrap();
                packages
                    .insert(&b"other.package"[..], &b"corrupt"[..])
                    .unwrap();
            }
            write.commit().unwrap();
            let read = reopened.begin_read().unwrap();
            assert_eq!(
                read_quarantined_snapshot(&read, package.package_name.as_bytes(), &key),
                Err(SnapshotRefusal::Corrupt)
            );
            drop(read);
            drop(reopened);
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn publication_quarantine_survives_process_exit_after_commit() {
    const CHILD_PATH: &str = "CAROSIGN_TEST_PUBLICATION_CRASH_DB";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        let db =
            crate::redb::backend::RedbStore::create_current(std::path::Path::new(&path)).unwrap();
        let record = sample();
        let package = PackageRecord {
            package_name: record.package_name.clone(),
            package_id: record.package_id.clone(),
            provider_id: record.provider_id.clone(),
            installed_version: record.version.clone(),
            approved_profiles: vec![RegisteredProfile {
                profile_name: record.profile_name.clone(),
                profile_id: record.profile_id.clone(),
            }],
        };
        let mut checks = 0;
        let _ = db.quarantine_initial_installation(&package, &record, || {
            checks += 1;
            if checks == 3 {
                // After the present commit, the owner's destructors are not executed.
                std::process::exit(73);
            }
            true
        });
        panic!("дочерний процесс не завершился в заданной точке");
    }

    let path = std::env::temp_dir().join(format!(
        "carosign-publication-exit-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "installation_records::tests::publication_quarantine_survives_process_exit_after_commit",
                "--exact",
                "--nocapture",
            ])
            .env(CHILD_PATH, &path)
            .status()
            .unwrap();
    assert_eq!(status.code(), Some(73));
    let record = sample();
    let key = [record.package_id.clone(), record.profile_id.clone()].concat();
    let before = std::fs::read(&path).unwrap();
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    let recovery = crate::redb::backend::QuarantineRecoveryStore::open(
        &path,
        record.package_name.as_bytes(),
        &key,
    )
    .unwrap();
    let snapshot = recovery.snapshot().unwrap();
    assert_eq!(snapshot.installation(), &record);
    assert!(recovery.revalidate(&snapshot).is_ok());
    assert!(
        crate::redb::backend::QuarantineRecoveryStore::open(
            &path,
            record.package_name.as_bytes(),
            &key
        )
        .is_err()
    );
    assert!(crate::redb::backend::RedbStore::open_current_guarded(&path).is_err());
    drop(recovery);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let link = path.with_extension("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(
        crate::redb::backend::QuarantineRecoveryStore::open(
            &link,
            record.package_name.as_bytes(),
            &key
        )
        .is_err()
    );
    std::fs::remove_file(link).unwrap();
    assert!(
        crate::redb::backend::QuarantineRecoveryStore::open(
            &path,
            record.package_name.as_bytes(),
            b"wrong"
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let corrupt = path.with_extension("corrupt");
    std::fs::copy(&path, &corrupt).unwrap();
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&corrupt)
            .unwrap();
        file.write_all(&[0; 64]).unwrap();
    }
    let corrupt_before = std::fs::read(&corrupt).unwrap();
    assert!(
        crate::redb::backend::QuarantineRecoveryStore::open(
            &corrupt,
            record.package_name.as_bytes(),
            &key
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&corrupt).unwrap(), corrupt_before);
    std::fs::remove_file(corrupt).unwrap();
    let db = redb::Database::open(&path).unwrap();
    assert_eq!(
        read_active_snapshot(
            &db.begin_read().unwrap(),
            record.package_name.as_bytes(),
            &key
        ),
        Err(SnapshotRefusal::UnsupportedSchema)
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn initial_package_and_installation_are_atomic_and_never_replace() {
    let path = std::env::temp_dir().join(format!(
        "carosign-initial-publication-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let key = [record.package_id.clone(), record.profile_id.clone()].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let tx = db.begin_write().unwrap();
    tx.open_table(META)
        .unwrap()
        .insert(SCHEMA_KEY, SCHEMA_V4)
        .unwrap();
    tx.open_table(PACKAGES).unwrap();
    tx.open_table(INSTALLATIONS).unwrap();
    tx.commit().unwrap();

    let mut bad = record.clone();
    bad.provider_id = vec![0x45; 16];
    assert_eq!(
        publish_initial_package_and_installation(&db, &package, &key, &bad),
        Err(PublicationFailure::Conflict)
    );
    let tx = db.begin_read().unwrap();
    assert!(
        tx.open_table(PACKAGES)
            .unwrap()
            .get(package.package_name.as_bytes())
            .unwrap()
            .is_none()
    );
    drop(tx);

    // A damaged third-party record should not allow the agreed candidate to be published over
    // an unverified snapshot.
    let write = db.begin_write().unwrap();
    write
        .open_table(INSTALLATIONS)
        .unwrap()
        .insert(&b"damaged"[..], &b"invalid"[..])
        .unwrap();
    write.commit().unwrap();
    assert_eq!(
        publish_initial_package_and_installation(&db, &package, &key, &record),
        Err(PublicationFailure::Corrupt)
    );
    let read = db.begin_read().unwrap();
    assert!(
        read.open_table(PACKAGES)
            .unwrap()
            .get(package.package_name.as_bytes())
            .unwrap()
            .is_none()
    );
    drop(read);
    let write = db.begin_write().unwrap();
    write
        .open_table(INSTALLATIONS)
        .unwrap()
        .remove(&b"damaged"[..])
        .unwrap();
    write.commit().unwrap();

    assert_eq!(
        publish_initial_package_and_installation(&db, &package, &key, &record),
        Err(PublicationFailure::RecoveryRequired)
    );
    assert_eq!(
        read_active_snapshot(
            &db.begin_read().unwrap(),
            package.package_name.as_bytes(),
            &key
        ),
        Err(SnapshotRefusal::UnsupportedSchema)
    );
    assert_eq!(
        publish_initial_package_and_installation(&db, &package, &key, &record),
        Err(PublicationFailure::UnsupportedSchema)
    );
    drop(db);
    let reopened = redb::Database::open(&path).unwrap();
    assert_eq!(
        read_active_snapshot(
            &reopened.begin_read().unwrap(),
            package.package_name.as_bytes(),
            &key
        ),
        Err(SnapshotRefusal::UnsupportedSchema)
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn migration_preflight_refuses_reused_package_identifiers() {
    let path = std::env::temp_dir().join(format!(
        "carosign-v3-package-identity-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    for collision in 0..4 {
        let tx = db.begin_write().unwrap();
        {
            tx.open_table(META)
                .unwrap()
                .insert(SCHEMA_KEY, crate::redb::backend::SCHEMA_V3)
                .unwrap();
            let mut packages = tx.open_table(PACKAGES).unwrap();
            let mut other = package.clone();
            other.package_name = "org.example.other".into();
            if collision != 0 {
                other.package_id = vec![11; 16];
            }
            if collision != 1 {
                other.provider_id = vec![12; 16];
            }
            if collision != 2 {
                other.approved_profiles[0].profile_id = vec![13; 16];
            }
            for entry in [&package, &other] {
                let bytes = envelope_with(
                    &StoredValue {
                        schema_version: 3,
                        value: Some(Value::PackageRecord(entry.clone())),
                    }
                    .encode_to_vec(),
                    sha256,
                )
                .unwrap();
                packages
                    .insert(entry.package_name.as_bytes(), bytes.as_slice())
                    .unwrap();
            }
        }
        tx.commit().unwrap();
        let tx = db.begin_read().unwrap();
        assert_eq!(
            preflight_v3_packages(&tx),
            if collision == 3 {
                Ok(2)
            } else {
                Err(SnapshotRefusal::Corrupt)
            }
        );
    }
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn publication_requires_revision_and_preserves_signer_and_transaction_boundary() {
    let path = std::env::temp_dir().join(format!(
        "carosign-publish-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let key = [vec![1; 16], vec![3; 16]].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_key = package.package_name.as_bytes();
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, SCHEMA_V4)
            .unwrap();
        let package_bytes = envelope_with(
            &StoredValue {
                schema_version: 4,
                value: Some(Value::PackageRecord(package.clone())),
            }
            .encode_to_vec(),
            sha256,
        )
        .unwrap();
        tx.open_table(PACKAGES)
            .unwrap()
            .insert(package_key, package_bytes.as_slice())
            .unwrap();
        tx.open_table(INSTALLATIONS).unwrap();
    }
    tx.commit().unwrap();
    let tx = db.begin_write().unwrap();
    assert_eq!(
        stage_publication(&tx, package_key, &key, &record, None),
        Ok(())
    );
    tx.abort().unwrap();
    let read = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&read, package_key, &key),
        Err(SnapshotRefusal::NotFound)
    );
    drop(read);
    let tx = db.begin_write().unwrap();
    stage_publication(&tx, package_key, &key, &record, None).unwrap();
    tx.commit().unwrap();
    let tx = db.begin_write().unwrap();
    let mut next = record.clone();
    next.revision += 1;
    assert_eq!(
        stage_publication(&tx, package_key, &key, &next, None),
        Err(PublicationFailure::Conflict)
    );
    let mut foreign = next.clone();
    foreign.signer_fingerprint[0] ^= 1;
    assert_eq!(
        stage_publication(&tx, package_key, &key, &foreign, Some(record.revision)),
        Err(PublicationFailure::Conflict)
    );
    assert_eq!(
        stage_publication(&tx, package_key, &key, &next, Some(record.revision)),
        Ok(())
    );
    tx.commit().unwrap();
    let read = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&read, package_key, &key).unwrap(),
        next
    );
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn v5_publication_requires_guard_and_survives_reopen() {
    use crate::redb::backend::InitialFenceStore;
    use crate::store::{Table, WriteTransaction};

    let path = std::env::temp_dir().join(format!(
        "carosign-v5-install-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let store = InitialFenceStore::create_new(&path).unwrap();
    let record = sample();
    let key = [vec![1; 16], vec![3; 16]].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_key = package.package_name.as_bytes();
    let encoded = envelope_with(
        &StoredValue {
            schema_version: 4,
            value: Some(Value::PackageRecord(package.clone())),
        }
        .encode_to_vec(),
        sha256,
    )
    .unwrap();
    let mut tx = store.begin_guarded_write().unwrap();
    tx.put(Table::Packages, package_key, &encoded).unwrap();
    assert_eq!(
        stage_publication(
            tx.installation_transaction().unwrap(),
            package_key,
            &key,
            &record,
            None
        ),
        Err(PublicationFailure::UnsupportedSchema)
    );
    stage_publication_v5(&mut tx, package_key, &key, &record, None).unwrap();
    Box::new(tx).commit_durable().unwrap();
    drop(store);
    let store = InitialFenceStore::open_guarded(&path).unwrap();
    drop(store);
    let db = redb::Database::open(&path).unwrap();
    let read = db.begin_read().unwrap();
    let installations = read.open_table(INSTALLATIONS).unwrap();
    let value = installations.get(key.as_slice()).unwrap().unwrap();
    assert_eq!(decode(&key, value.value()).unwrap(), record);
    drop(value);
    drop(installations);
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn uninstall_revokes_all_profiles_and_version_atomically() {
    use carosign_core::ids::ProfileId;
    let path = std::env::temp_dir().join(format!(
        "carosign-uninstall-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let first = sample();
    let mut second = first.clone();
    second.profile_id = vec![13; 16];
    second.profile_name = "other".to_string();
    second.revision = 7;
    let first_key = [first.package_id.as_slice(), first.profile_id.as_slice()].concat();
    let second_key = [second.package_id.as_slice(), second.profile_id.as_slice()].concat();
    let package = PackageRecord {
        package_name: first.package_name.clone(),
        package_id: first.package_id.clone(),
        provider_id: first.provider_id.clone(),
        installed_version: first.version.clone(),
        approved_profiles: vec![
            RegisteredProfile {
                profile_name: first.profile_name.clone(),
                profile_id: first.profile_id.clone(),
            },
            RegisteredProfile {
                profile_name: second.profile_name.clone(),
                profile_id: second.profile_id.clone(),
            },
        ],
    };
    let package_key = package.package_name.as_bytes();
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, SCHEMA_V4)
            .unwrap();
        let bytes = envelope_with(
            &StoredValue {
                schema_version: 4,
                value: Some(Value::PackageRecord(package.clone())),
            }
            .encode_to_vec(),
            sha256,
        )
        .unwrap();
        tx.open_table(PACKAGES)
            .unwrap()
            .insert(package_key, bytes.as_slice())
            .unwrap();
        let mut records = tx.open_table(INSTALLATIONS).unwrap();
        records
            .insert(
                first_key.as_slice(),
                encode(&first_key, &first).unwrap().as_slice(),
            )
            .unwrap();
        records
            .insert(
                second_key.as_slice(),
                encode(&second_key, &second).unwrap().as_slice(),
            )
            .unwrap();
    }
    tx.commit().unwrap();
    let expected = [
        (ProfileId::from_bytes([3; 16]).unwrap(), first.revision),
        (ProfileId::from_bytes([13; 16]).unwrap(), second.revision),
    ];
    let tx = db.begin_write().unwrap();
    assert_eq!(
        stage_uninstall(&tx, package_key, &expected[..1]),
        Err(RevokeFailure::Conflict)
    );
    assert_eq!(
        stage_uninstall(&tx, package_key, &[expected[0], expected[0]]),
        Err(RevokeFailure::Conflict)
    );
    assert_eq!(
        stage_uninstall(&tx, package_key, &[expected[0], (expected[1].0, 8)]),
        Err(RevokeFailure::Conflict)
    );
    tx.commit().unwrap();
    let before = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&before, package_key, &first_key).unwrap(),
        first
    );
    assert_eq!(
        read_active_snapshot(&before, package_key, &second_key).unwrap(),
        second
    );
    let tx = db.begin_write().unwrap();
    {
        let mut table = tx.open_table(INSTALLATIONS).unwrap();
        let mut overflow = second.clone();
        overflow.revision = u64::MAX;
        table
            .insert(
                second_key.as_slice(),
                encode(&second_key, &overflow).unwrap().as_slice(),
            )
            .unwrap();
    }
    assert_eq!(
        stage_uninstall(&tx, package_key, &[expected[0], (expected[1].0, u64::MAX)]),
        Err(RevokeFailure::RevisionExhausted)
    );
    {
        let table = tx.open_table(INSTALLATIONS).unwrap();
        let first_after = table.get(first_key.as_slice()).unwrap().unwrap();
        assert_eq!(decode(&first_key, first_after.value()).unwrap(), first);
    }
    tx.abort().unwrap();
    let tx = db.begin_write().unwrap();
    assert_eq!(stage_uninstall(&tx, package_key, &expected), Ok(()));
    tx.abort().unwrap();
    let read = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&read, package_key, &first_key).unwrap(),
        first
    );
    drop(read);
    let tx = db.begin_write().unwrap();
    stage_uninstall(&tx, package_key, &expected).unwrap();
    tx.commit().unwrap();
    // Open before fixing the picture retains the previous agreed version.
    assert_eq!(
        read_active_snapshot(&before, package_key, &first_key).unwrap(),
        first
    );
    drop(before);
    let read = db.begin_read().unwrap();
    let packages = read.open_table(PACKAGES).unwrap();
    let current_package = decode_package(
        package_key,
        packages.get(package_key).unwrap().unwrap().value(),
    )
    .unwrap();
    assert!(current_package.installed_version.is_empty());
    assert_eq!(current_package.approved_profiles, package.approved_profiles);
    let installations = read.open_table(INSTALLATIONS).unwrap();
    for (key, original) in [(&first_key, first), (&second_key, second)] {
        let current = decode(
            key,
            installations.get(key.as_slice()).unwrap().unwrap().value(),
        )
        .unwrap();
        assert_eq!(current.state, 2);
        assert_eq!(current.revision, original.revision + 1);
        assert_eq!(current.profile_id, original.profile_id);
        check_package(key, &current, &current_package).unwrap();
    }
    drop(installations);
    drop(packages);
    drop(read);
    let tx = db.begin_write().unwrap();
    assert_eq!(
        stage_uninstall(&tx, package_key, &expected),
        Err(RevokeFailure::Conflict)
    );
    tx.abort().unwrap();
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn uninstall_already_revoked_package_keeps_the_record_and_revision() {
    let path = std::env::temp_dir().join(format!(
        "carosign-uninstall-revoked-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let mut record = sample();
    record.state = 2;
    let key = [record.package_id.as_slice(), record.profile_id.as_slice()].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_key = package.package_name.as_bytes();
    let tx = db.begin_write().unwrap();
    {
        tx.open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, SCHEMA_V4)
            .unwrap();
        let bytes = envelope_with(
            &StoredValue {
                schema_version: 4,
                value: Some(Value::PackageRecord(package.clone())),
            }
            .encode_to_vec(),
            sha256,
        )
        .unwrap();
        tx.open_table(PACKAGES)
            .unwrap()
            .insert(package_key, bytes.as_slice())
            .unwrap();
        tx.open_table(INSTALLATIONS)
            .unwrap()
            .insert(key.as_slice(), encode(&key, &record).unwrap().as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    let tx = db.begin_write().unwrap();
    stage_uninstall(&tx, package_key, &[]).unwrap();
    tx.commit().unwrap();
    let read = db.begin_read().unwrap();
    let packages = read.open_table(PACKAGES).unwrap();
    let current = decode_package(
        package_key,
        packages.get(package_key).unwrap().unwrap().value(),
    )
    .unwrap();
    assert!(current.installed_version.is_empty());
    let records = read.open_table(INSTALLATIONS).unwrap();
    assert_eq!(
        decode(&key, records.get(key.as_slice()).unwrap().unwrap().value()).unwrap(),
        record
    );
    drop(records);
    drop(packages);
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn revocation_refuses_missing_tables_wrong_schema_and_revision_overflow() {
    let path = std::env::temp_dir().join(format!(
        "carosign-v4-revocation-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = redb::Database::create(&path).unwrap();
    let record = sample();
    let key = [vec![1; 16], vec![3; 16]].concat();
    let package = PackageRecord {
        package_name: record.package_name.clone(),
        package_id: record.package_id.clone(),
        provider_id: record.provider_id.clone(),
        installed_version: record.version.clone(),
        approved_profiles: vec![RegisteredProfile {
            profile_name: record.profile_name.clone(),
            profile_id: record.profile_id.clone(),
        }],
    };
    let package_key = package.package_name.as_bytes();
    let write = db.begin_write().unwrap();
    assert_eq!(
        stage_revocation(&write, package_key, &key, 1),
        Err(RevokeFailure::Corrupt)
    );
    write.commit().unwrap();
    let read = db.begin_read().unwrap();
    assert!(matches!(
        read.open_table(INSTALLATIONS),
        Err(redb::TableError::TableDoesNotExist(_))
    ));
    drop(read);
    let write = db.begin_write().unwrap();
    {
        let mut meta = write.open_table(META).unwrap();
        meta.insert(SCHEMA_KEY, crate::redb::backend::SCHEMA_V3)
            .unwrap();
        let mut packages = write.open_table(PACKAGES).unwrap();
        let value = envelope_with(
            &StoredValue {
                schema_version: 4,
                value: Some(Value::PackageRecord(package.clone())),
            }
            .encode_to_vec(),
            sha256,
        )
        .unwrap();
        packages.insert(package_key, value.as_slice()).unwrap();
        let mut installations = write.open_table(INSTALLATIONS).unwrap();
        let mut last = record.clone();
        last.revision = u64::MAX;
        let value = encode(&key, &last).unwrap();
        installations
            .insert(key.as_slice(), value.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    let write = db.begin_write().unwrap();
    assert_eq!(
        stage_revocation(&write, package_key, &key, u64::MAX),
        Err(RevokeFailure::UnsupportedSchema)
    );
    write.abort().unwrap();
    let write = db.begin_write().unwrap();
    {
        write
            .open_table(META)
            .unwrap()
            .insert(SCHEMA_KEY, SCHEMA_V4)
            .unwrap();
    }
    write.commit().unwrap();
    let write = db.begin_write().unwrap();
    assert_eq!(
        stage_revocation(&write, package_key, &key, u64::MAX),
        Err(RevokeFailure::RevisionExhausted)
    );
    write.commit().unwrap();
    let read = db.begin_read().unwrap();
    assert_eq!(
        read_active_snapshot(&read, package_key, &key)
            .unwrap()
            .revision,
        u64::MAX
    );
    drop(read);
    drop(db);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn v4_metadata_accepts_only_well_formed_committed_migration() {
    use redb::Database;
    let path = std::env::temp_dir().join(format!(
        "carosign-v4-committed-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let db = Database::create(&path).unwrap();
    let key: &[u8] = b"biometric.migration.v3-v4";
    let mut payload = [0u8; 128];
    payload[..4].copy_from_slice(b"BMIG");
    payload[4] = 1;
    payload[5] = 3;
    payload[8..12].copy_from_slice(&3u32.to_be_bytes());
    payload[12..16].copy_from_slice(&4u32.to_be_bytes());
    payload[16..32].fill(1);
    payload[32..40].copy_from_slice(&1u64.to_be_bytes());
    payload[40..72].fill(2);
    payload[80..88].copy_from_slice(&1u64.to_be_bytes());
    payload[96..128].copy_from_slice(&sha256(&[]));
    let envelope = |bytes: &[u8]| envelope_with(bytes, sha256).unwrap();
    let write = db.begin_write().unwrap();
    {
        let mut table = write.open_table(META).unwrap();
        table.insert(SCHEMA_KEY, SCHEMA_V4).unwrap();
        table.insert(key, envelope(&payload).as_slice()).unwrap();
        assert_eq!(normal_v4_meta(&table), Ok(true));
        payload[5] = 2;
        table.insert(key, envelope(&payload).as_slice()).unwrap();
        assert_eq!(normal_v4_meta(&table), Ok(false));
        payload[5] = 3;
        let mut damaged = envelope(&payload);
        *damaged.last_mut().unwrap() ^= 1;
        table.insert(key, damaged.as_slice()).unwrap();
        assert_eq!(normal_v4_meta(&table), Ok(false));
        table.insert(key, envelope(&payload).as_slice()).unwrap();
        table.insert(&b"unexpected"[..], &b"x"[..]).unwrap();
        assert_eq!(normal_v4_meta(&table), Ok(false));
    }
    write.abort().unwrap();
    drop(db);
    std::fs::remove_file(path).unwrap();
}
