use super::*;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthorityId, EnrollmentId, SubjectId};

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

#[test]
fn scope_is_forty_eight_bytes() {
    let scope = scope_of(
        subject(),
        AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
    );
    assert_eq!(scope.len(), 48);
    assert_eq!(&scope[..32], &subject().to_canonical());
}

#[test]
fn selector_current_is_only_current() {
    assert!(selector_is_current(&SubjectSelector::Current));
    assert!(!selector_is_current(&SubjectSelector::AccountHint(
        carosign_core::identity::AccountHint::new("alex").expect("годится")
    )));
}

#[test]
fn errors_sound_different() {
    // ⚠️ Three troubles, three words.
    assert_ne!(
        AcceptError::NotEnrolled.to_string(),
        AcceptError::Unauthorized(AuthorityRefusal::Denied).to_string()
    );
    assert_ne!(
        AcceptError::IdentityUnavailable.to_string(),
        AcceptError::NotEnrolled.to_string()
    );
    let _ = EnrollmentRef {
        id: EnrollmentId::from_bytes([4u8; 16]).expect("валид"),
        authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
        generation: EnrollmentGeneration::FIRST,
    };
}

#[test]
fn missing_reverse_index_cannot_hide_enrollment() {
    use carosign_persistence::store::MetadataStore;
    let store = carosign_persistence::test_support::MemoryStore::default();
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let record = carosign_persistence::enrollment_codec::EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([4; 16]).unwrap(),
            authenticator,
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "test".into(),
        status: carosign_persistence::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![5]).unwrap(),
        format_id: "mock".into(),
        format_version: 1,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        record.enrollment.id.as_bytes(),
        &carosign_persistence::enrollment_codec::encode_v3(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(read_enrollment_records(&store, &scope_of(subject(), authenticator)).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::EnrollmentIndex,
        &carosign_persistence::redb::codec::enrollment_index_key(&record),
        record.enrollment.id.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        read_enrollment_records(&store, &scope_of(subject(), authenticator))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn v4_scope_reads_v4_records_and_rejects_v3_record() {
    use carosign_persistence::store::MetadataStore;
    let path = std::env::temp_dir().join(format!("carosign-v4-read-{}.redb", std::process::id()));
    let store = carosign_persistence::redb::backend::RedbStore::create_current(&path).unwrap();
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let record = carosign_persistence::enrollment_codec::EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([4; 16]).unwrap(),
            authenticator,
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "test".into(),
        status: carosign_persistence::enrollment_codec::EnrollmentStatus::Active,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![5]).unwrap(),
        format_id: "mock".into(),
        format_version: 1,
        binding_kind: carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        record.enrollment.id.as_bytes(),
        &carosign_persistence::enrollment_codec::encode_v3(&record).unwrap(),
    )
    .unwrap();
    tx.put(
        Table::EnrollmentIndex,
        &carosign_persistence::redb::codec::enrollment_index_key(&record),
        record.enrollment.id.as_bytes(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(read_enrollment_records(&store, &scope_of(subject(), authenticator)).is_err());
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        record.enrollment.id.as_bytes(),
        &carosign_persistence::mutation_store::encode_enrollment_v4(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        read_enrollment_records(&store, &scope_of(subject(), authenticator))
            .unwrap()
            .len(),
        1
    );
    drop(store);
    std::fs::remove_file(path).unwrap();
}
