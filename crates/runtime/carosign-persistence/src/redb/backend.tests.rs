use super::preflight::PREFLIGHT_WRITE_LIMIT;
use super::*;

#[test]
fn scoped_fence_survives_reopen_and_rejects_partial_set() {
    use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
    use carosign_core::ids::{
        AuthenticatorId, LeaseId, OperationId, ResourceId, WorkerInstanceId,
    };
    use carosign_core::resource::{AccessMode, ResourceClaim, ResourceSet};
    let path = test_path("scoped-fence");
    let first = ResourceId::from_bytes([1; 16]).unwrap();
    let second = ResourceId::from_bytes([2; 16]).unwrap();
    let scope = FenceScope {
        lease: LeaseId::from_bytes([3; 16]).unwrap(),
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        worker: WorkerInstanceId::from_bytes([5; 16]).unwrap(),
        authenticator: AuthenticatorRef {
            id: AuthenticatorId::from_bytes([6; 16]).unwrap(),
            device_epoch: DeviceEpoch::FIRST,
        },
        resources: ResourceSet::new(vec![
            ResourceClaim {
                id: second,
                mode: AccessMode::Exclusive,
            },
            ResourceClaim {
                id: first,
                mode: AccessMode::Exclusive,
            },
        ])
        .unwrap(),
    };
    let store = InitialFenceStore::create_new(&path).unwrap();
    store.fence_scoped(&scope).unwrap();
    drop(store);
    let store = InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(store.fences().unwrap(), vec![first, second]);
    let recovered = store.inspect_recovery_scope(scope.operation).unwrap();
    assert_eq!(recovered.lease, scope.lease);
    assert_eq!(recovered.worker, scope.worker);
    assert_eq!(recovered.authenticator, scope.authenticator);
    assert_eq!(recovered.resources, scope.resources);
    assert!(
        store
            .inspect_recovery_scope(OperationId::from_bytes([9; 16]).unwrap())
            .is_err()
    );
    let tx = store.database.begin_read().unwrap();
    let table = tx.open_table(RESOURCE_FENCES).unwrap();
    assert_eq!(
        table
            .get(first.as_bytes().as_slice())
            .unwrap()
            .unwrap()
            .value(),
        scope.encode()
    );
    drop(table);
    drop(tx);
    assert!(store.fence_scoped(&scope).is_err());
    let wrong = FenceScope {
        lease: LeaseId::from_bytes([8; 16]).unwrap(),
        operation: scope.operation,
        worker: scope.worker,
        authenticator: scope.authenticator,
        resources: scope.resources.clone(),
    };
    // SAFETY: The fake proof is only created in the test; there are no devices.
    let permit = unsafe { DurableReleasePermit::from_verified_quiescence(wrong) };
    assert!(store.release_verified(permit).is_err());
    assert_eq!(store.fences().unwrap(), vec![first, second]);
    let tx = store.database.begin_write().unwrap();
    {
        let mut table = tx.open_table(RESOURCE_FENCES).unwrap();
        table.remove(first.as_bytes().as_slice()).unwrap();
    }
    tx.commit().unwrap();
    assert!(store.inspect_recovery_scope(scope.operation).is_err());
    assert!(store.fences().is_err());
    drop(store);
    assert!(InitialFenceStore::open_guarded(&path).is_err());
    std::fs::remove_file(path).unwrap();
}

fn test_path(name: &str) -> std::path::PathBuf {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("carosign-{name}-{}-{time}", std::process::id()))
}

#[test]
fn initial_fences_survive_reopen_and_v4_stays_unchanged() {
    let v5_path = test_path("fences-v5");
    let v4_path = test_path("fences-v4");
    let v4 = RedbStore::create_current(&v4_path).unwrap();
    drop(v4);
    let before = std::fs::read(&v4_path).unwrap();
    assert!(InitialFenceStore::create_new(&v4_path).is_err());
    assert!(InitialFenceStore::open_guarded(&v4_path).is_err());
    assert_eq!(before, std::fs::read(&v4_path).unwrap());
    let resource = carosign_core::ids::ResourceId::from_bytes([7; 16]).unwrap();
    let store = InitialFenceStore::create_new(&v5_path).unwrap();
    assert!(store.fences().unwrap().is_empty());
    store.fence(resource).unwrap();
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&v5_path).unwrap();
    assert_eq!(reopened.fences().unwrap(), vec![resource]);
    drop(reopened);
    std::fs::remove_file(v4_path).unwrap();
    std::fs::remove_file(v5_path).unwrap();
}

#[test]
fn resource_set_is_fenced_together_and_invalid_set_changes_nothing() {
    use carosign_core::ids::ResourceId;
    let path = test_path("fence-set");
    let first = ResourceId::from_bytes([1; 16]).unwrap();
    let second = ResourceId::from_bytes([2; 16]).unwrap();
    let store = InitialFenceStore::create_new(&path).unwrap();
    assert!(store.fence_resources(&[]).is_err());
    assert!(store.fence_resources(&[first, first]).is_err());
    let too_many = (1..=9)
        .map(|n| ResourceId::from_bytes([n; 16]).unwrap())
        .collect::<Vec<_>>();
    assert!(store.fence_resources(&too_many).is_err());
    assert!(store.fences().unwrap().is_empty());
    store.fence_resources(&[first, second]).unwrap();
    let third = ResourceId::from_bytes([3; 16]).unwrap();
    assert!(store.fence_resources(&[third, first]).is_err());
    assert_eq!(store.fences().unwrap(), vec![first, second]);
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    assert_eq!(reopened.fences().unwrap(), vec![first, second]);
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn malformed_fence_fails_preflight_without_changing_file() {
    let path = test_path("fences-corrupt");
    let store = InitialFenceStore::create_new(&path).unwrap();
    let tx = store.database.begin_write().unwrap();
    {
        tx.open_table(RESOURCE_FENCES)
            .unwrap()
            .insert([0u8; 16].as_slice(), RESOURCE_FENCE_VALUE)
            .unwrap();
    }
    tx.commit().unwrap();
    drop(store);
    let before = std::fs::read(&path).unwrap();
    assert!(InitialFenceStore::open_guarded(&path).is_err());
    assert_eq!(before, std::fs::read(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn v5_preflight_checks_nonempty_registry_and_its_index() {
    use carosign_core::{
        enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef},
        identity::SubjectRef,
        ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId},
    };
    let path = test_path("fences-registry");
    let store = InitialFenceStore::create_new(&path).unwrap();
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
    let encoded = crate::mutation_store::encode_enrollment_v4(&record).unwrap();
    let index = crate::redb::codec::enrollment_index_key(&record);
    let tx = store.database.begin_write().unwrap();
    {
        tx.open_table(ENROLLMENTS)
            .unwrap()
            .insert(key.as_slice(), encoded.as_slice())
            .unwrap();
        tx.open_table(ENROLLMENT_INDEX)
            .unwrap()
            .insert(index.as_slice(), key.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(store);
    let store = InitialFenceStore::open_guarded(&path).unwrap();
    let tx = store.database.begin_write().unwrap();
    {
        tx.open_table(ENROLLMENT_INDEX)
            .unwrap()
            .remove(index.as_slice())
            .unwrap();
    }
    tx.commit().unwrap();
    drop(store);
    let before = std::fs::read(&path).unwrap();
    assert!(InitialFenceStore::open_guarded(&path).is_err());
    assert_eq!(before, std::fs::read(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn second_open_is_refused_and_replacement_cannot_receive_confirmation() {
    let path = test_path("fences-lock");
    let moved = test_path("fences-moved");
    let store = InitialFenceStore::create_new(&path).unwrap();
    assert!(InitialFenceStore::open_guarded(&path).is_err());
    std::fs::rename(&path, &moved).unwrap();
    let replacement = InitialFenceStore::create_new(&path).unwrap();
    let resource = carosign_core::ids::ResourceId::from_bytes([9; 16]).unwrap();
    assert!(store.fence(resource).is_err());
    assert!(store.fences().is_err());
    assert!(replacement.fences().unwrap().is_empty());
    drop(store);
    drop(replacement);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(moved).unwrap();
}

#[test]
fn guarded_transaction_refuses_receipt_if_file_replaced_after_begin() {
    let path = test_path("guarded-transaction");
    let moved = test_path("guarded-transaction-old");
    let store = InitialFenceStore::create_new(&path).unwrap();
    let mut tx = store.begin_guarded_write().unwrap();
    tx.put(Table::Packages, b"key", b"value").unwrap();
    std::fs::rename(&path, &moved).unwrap();
    let replacement = InitialFenceStore::create_new(&path).unwrap();
    assert!(Box::new(tx).commit_durable().is_err());
    assert!(replacement.fences().unwrap().is_empty());
    drop(store);
    drop(replacement);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(moved).unwrap();
}

#[test]
fn overlay_never_writes_base_and_truncation_then_growth_reads_zeroes() {
    let path = std::env::temp_dir().join(format!("carosign-preflight-{}", std::process::id()));
    std::fs::write(&path, b"abcdef").unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let base = std::sync::Arc::new(redb::backends::FileBackend::new(file).unwrap());
    let overlay = PreflightBackend::new(base.clone()).unwrap();
    overlay.write(1, b"XY").unwrap();
    assert_eq!(overlay.read(0, 6).unwrap(), b"aXYdef");
    overlay.set_len(3).unwrap();
    overlay.set_len(6).unwrap();
    assert_eq!(overlay.read(0, 6).unwrap(), b"aXY\0\0\0");
    assert!(
        overlay
            .write(0, &vec![0; PREFLIGHT_WRITE_LIMIT + 1])
            .is_err()
    );
    drop(overlay);
    drop(base);
    assert_eq!(std::fs::read(&path).unwrap(), b"abcdef");
    std::fs::remove_file(path).unwrap();
}
