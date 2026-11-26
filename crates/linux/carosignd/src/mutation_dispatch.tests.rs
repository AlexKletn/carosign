use super::*;
use carosign_protocol::carosign::common::v1::{SubjectSelector, subject_selector::Selector};
use carosign_runtime::ports::TransactionId;
use std::sync::Arc;

#[test]
fn enrollment_requires_its_own_action_and_exact_target() {
    let _lock = crate::policy_test_lock();
    let dir = std::path::PathBuf::from(format!(
        "/dev/shm/carosign-mutation-policy-{}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, "uid=0 subject=0 actions=enroll\n").unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&dir.join("db")).unwrap();
    let policy = crate::linux_policy::LinuxPolicy::fixture(&path, clock);
    let caller =
        CallerContext::from_transport([1; 32], 0, TransactionId::from_bytes([2; 16]).unwrap());
    let auth = AuthenticatorId::from_bytes([5; 16]).unwrap();
    let selector = SubjectSelector {
        selector: Some(Selector::AccountHint("root".into())),
    };
    let enroll = DomainRequest::BeginEnrollment {
        idempotency: IdempotencyKey::from_bytes([3; 16]).unwrap(),
        authenticator: auth,
        label: "left-thumb".into(),
        subject: selector.clone(),
        replace: None,
        authority_request_ref: vec![],
    };
    let prepared =
        futures_executor::block_on(prepare(&store, &policy, &caller, &enroll, auth)).unwrap();
    assert!(matches!(
        prepared.grant.scope(),
        GrantScope::Enroll { replace: None, .. }
    ));
    assert_eq!(prepared.plan.requested_label.as_deref(), Some("left-thumb"));
    assert!(
        prepared
            .plan
            .to_core(carosign_persistence::mutation_plan::Scope {
                subject: prepared.subject,
                authenticator: prepared.authenticator,
            })
            .is_ok()
    );
    let repeated =
        futures_executor::block_on(prepare(&store, &policy, &caller, &enroll, auth)).unwrap();
    assert_eq!(prepared.request_digest, repeated.request_digest);
    let mut changed = enroll.clone();
    if let DomainRequest::BeginEnrollment { label, .. } = &mut changed {
        *label = "right-thumb".into();
    }
    let changed =
        futures_executor::block_on(prepare(&store, &policy, &caller, &changed, auth)).unwrap();
    assert_ne!(prepared.request_digest, changed.request_digest);
    let remove = DomainRequest::BeginRemoval {
        idempotency: IdempotencyKey::from_bytes([4; 16]).unwrap(),
        authenticator: auth,
        enrollment: EnrollmentId::from_bytes([6; 16]).unwrap(),
        generation: 1,
        subject: selector,
        authority_request_ref: vec![],
    };
    assert!(matches!(
        futures_executor::block_on(prepare(&store, &policy, &caller, &remove, auth)),
        Err(MutationPrepareError::Denied)
    ));
    let foreign =
        CallerContext::from_transport([7; 32], 1, TransactionId::from_bytes([8; 16]).unwrap());
    assert!(matches!(
        futures_executor::block_on(prepare(&store, &policy, &foreign, &enroll, auth)),
        Err(MutationPrepareError::Denied)
    ));
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn removal_requires_matching_live_generation_in_same_subject_scope() {
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::AuthorityId;
    use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    use carosign_persistence::mutation_plan::StoredBindingKind;
    use carosign_persistence::store::{MetadataStore, Table};
    let _lock = crate::policy_test_lock();
    let dir = std::path::PathBuf::from(format!(
        "/dev/shm/carosign-removal-policy-{}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, "uid=0 subject=0 actions=remove\n").unwrap();
    let store =
        carosign_persistence::redb::backend::RedbStore::create_current(&dir.join("db")).unwrap();
    let clock = Arc::new(carosign_adapters::clock::StepClock::new(
        carosign_core::ids::BootId::from_bytes([1; 16]).unwrap(),
    ));
    let policy = crate::linux_policy::LinuxPolicy::fixture(&path, clock);
    let auth = AuthenticatorId::from_bytes([5; 16]).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes(*b"linux-local-v001").unwrap(),
        subject: carosign_adapters::policy::subject_of(0),
    };
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([6; 16]).unwrap(),
        authenticator: auth,
        generation: EnrollmentGeneration::FIRST,
    };
    let record = EnrollmentRecord {
        subject,
        enrollment,
        display_label: None,
        label: "left-thumb".into(),
        status: EnrollmentStatus::Active,
        provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(vec![21; 16]).unwrap(),
        format_id: String::new(),
        format_version: 0,
        binding_kind: StoredBindingKind::ProviderOpaque,
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Enrollments,
        enrollment.id.as_bytes(),
        &carosign_persistence::mutation_store::encode_enrollment_v4(&record).unwrap(),
    )
    .unwrap();
    let mut index = crate::accept::scope_of(subject, auth).to_vec();
    index.extend_from_slice(enrollment.id.as_bytes());
    tx.put(Table::EnrollmentIndex, &index, enrollment.id.as_bytes())
        .unwrap();
    tx.commit_durable().unwrap();
    let caller =
        CallerContext::from_transport([1; 32], 0, TransactionId::from_bytes([2; 16]).unwrap());
    let mut remove = DomainRequest::BeginRemoval {
        idempotency: IdempotencyKey::from_bytes([4; 16]).unwrap(),
        authenticator: auth,
        enrollment: enrollment.id,
        generation: 2,
        subject: SubjectSelector {
            selector: Some(Selector::AccountHint("root".into())),
        },
        authority_request_ref: vec![],
    };
    assert!(matches!(
        futures_executor::block_on(prepare(&store, &policy, &caller, &remove, auth)),
        Err(MutationPrepareError::Conflict)
    ));
    if let DomainRequest::BeginRemoval { generation, .. } = &mut remove {
        *generation = 1;
    }
    let prepared =
        futures_executor::block_on(prepare(&store, &policy, &caller, &remove, auth)).unwrap();
    assert!(
        matches!(prepared.grant.scope(), GrantScope::Remove { enrollment: value, .. } if *value == enrollment)
    );
    assert_eq!(prepared.plan.old_target, Some(enrollment));
    assert_eq!(prepared.plan.pre_bindings.len(), 1);
    assert_eq!(prepared.plan.requested_label, None);
    drop(policy);
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}
