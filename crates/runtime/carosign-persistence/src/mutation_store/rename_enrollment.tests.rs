use carosign_core::{
    enrollment::{EnrollmentGeneration, OpaqueProviderRef},
    ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId},
};

use crate::{
    enrollment_codec::EnrollmentRecord, mutation_plan::StoredBindingKind,
    store::tests_support::MemoryStore,
};

use super::*;

#[test]
fn rejects_other_subjects_locked_scopes_and_revoked_writes() {
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let other = SubjectRef {
        subject: SubjectId::from_bytes([3; 16]).unwrap(),
        ..subject
    };
    let target = EnrollmentRef {
        id: EnrollmentId::from_bytes([4; 16]).unwrap(),
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment: target,
        label: "left-thumb".into(),
        display_label: None,
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![6]).unwrap(),
        format_id: "test".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let store = MemoryStore::default();
    let key = target.id.as_bytes();
    let index = enrollment_index_key(&record);
    let scope = &index[..SCOPE_LOCK_KEY_LEN];
    let original = encode_enrollment_v4(&record).unwrap();
    let mut transaction = store.begin_write().unwrap();
    transaction.put(Table::Meta, SCHEMA_KEY, SCHEMA_V4).unwrap();
    transaction.put(Table::Enrollments, key, &original).unwrap();
    transaction
        .put(Table::EnrollmentIndex, &index, key)
        .unwrap();
    transaction.commit_durable().unwrap();
    let expected = EnrollmentLabel::new("left-thumb").unwrap();
    let renamed = EnrollmentLabel::new("My thumb").unwrap();

    assert_eq!(
        rename_enrollment(&store, other, target, &expected, &renamed, || Ok(())),
        Err(StorageError::GenerationConflict)
    );
    assert_eq!(
        rename_enrollment(&store, subject, target, &expected, &renamed, || {
            Err(StorageError::Unavailable)
        }),
        Err(StorageError::Unavailable)
    );
    let mut transaction = store.begin_write().unwrap();
    transaction
        .put(Table::MutationScopeLock, scope, &[7; 16])
        .unwrap();
    transaction.commit_durable().unwrap();
    assert_eq!(
        rename_enrollment(&store, subject, target, &expected, &renamed, || Ok(())),
        Err(StorageError::GenerationConflict)
    );
    let read = store.begin_read().unwrap();
    assert_eq!(read.get(Table::Enrollments, key).unwrap(), Some(original));
}
