//! Acceptance of the request: from the registry to the verification cycle.
//!
//! ⚠️ The service’s live path is checked: the set is assembled ** from the registry **, not
//! out of thin air, and the three troubles sound different.

#[path = "accept_flow_fixtures/allow.rs"]
mod allow;
#[path = "accept_flow_fixtures/change_during_authorization.rs"]
mod change_during_authorization;
#[path = "accept_flow_fixtures/deny.rs"]
mod deny;
#[path = "accept_flow_fixtures/lock_during_authorization.rs"]
mod lock_during_authorization;

use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, BootId, EnrollmentId, GrantId, SubjectId};
use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
use carosign_persistence::mutation_plan::StoredBindingKind;
use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::store::{MetadataStore, Table};
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, PolicyAuthority, TransactionId,
};
use carosignd::accept::{AcceptError, prepare_verification, scope_of};
use std::path::PathBuf;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("accept-{name}-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn caller() -> CallerContext {
    CallerContext::from_transport(
        [0u8; 32],
        1000,
        TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
    )
}

fn record(byte: u8, status: EnrollmentStatus) -> EnrollmentRecord {
    EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status,
        provider_ref: OpaqueProviderRef::new(vec![byte; 4]).expect("годится"),
        format_id: "goodix-asp".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    }
}

fn put(store: &RedbStore, record: &EnrollmentRecord) {
    let mut tx = store.begin_write().expect("запись");
    let bytes = carosign_persistence::enrollment_codec::encode_v3(record).expect("кодируется");
    tx.put(Table::Enrollments, record.enrollment.id.as_bytes(), &bytes)
        .expect("положено");
    let mut index = [0u8; 64];
    index[..32].copy_from_slice(&subject().to_canonical());
    index[32..48].copy_from_slice(authenticator().as_bytes());
    index[48..].copy_from_slice(record.enrollment.id.as_bytes());
    tx.put(
        Table::EnrollmentIndex,
        &index,
        record.enrollment.id.as_bytes(),
    )
    .expect("индекс");
    tx.commit_durable().expect("зафиксировано");
}

use allow::Allow;

use deny::Deny;

use lock_during_authorization::LockDuringAuthorization;

use change_during_authorization::ChangeDuringAuthorization;

#[test]
fn empty_registry_is_not_enrolled() {
    let path = temp_path("empty");
    let store = RedbStore::create(&path).expect("создано");
    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert_eq!(result.err(), Some(AcceptError::NotEnrolled));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn active_enrollment_reaches_the_cycle() {
    let path = temp_path("active");
    let store = RedbStore::create(&path).expect("создано");
    put(&store, &record(5, EnrollmentStatus::Active));

    let cycle = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ))
    .expect("цикл");
    match cycle.plan() {
        carosign_runtime::ports::CommandPlan::Verify { permitted } => {
            assert_eq!(permitted.len(), 1)
        }
        other => panic!("ожидалась проверка, а пришло {other:?}"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn old_envelope_is_not_accepted_as_v3() {
    let path = temp_path("old-envelope");
    let store = RedbStore::create(&path).expect("создано");
    let entry = record(5, EnrollmentStatus::Active);
    put(&store, &entry);
    let old = carosign_persistence::enrollment_codec::encode(&entry).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::Enrollments, entry.enrollment.id.as_bytes(), &old)
        .unwrap();
    tx.commit_durable().unwrap();

    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert_eq!(result.err(), Some(AcceptError::IdentityUnavailable));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn index_pointing_to_another_subject_or_authenticator_is_rejected() {
    for (name, change_subject) in [("foreign-subject", true), ("foreign-device", false)] {
        let path = temp_path(name);
        let store = RedbStore::create(&path).expect("создано");
        let mut entry = record(5, EnrollmentStatus::Active);
        if change_subject {
            entry.subject.subject = SubjectId::from_bytes([9; 16]).unwrap();
        } else {
            entry.enrollment.authenticator = AuthenticatorId::from_bytes([9; 16]).unwrap();
        }
        // put intentionally creates an index of the current subject, indicating someone
        // else's record.
        put(&store, &entry);
        let result = futures_executor::block_on(prepare_verification(
            &store,
            &Allow,
            &caller(),
            subject(),
            authenticator(),
            DeviceEpoch::FIRST,
        ));
        assert_eq!(result.err(), Some(AcceptError::IdentityUnavailable));
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn policy_refusal_is_its_own_word() {
    let path = temp_path("deny");
    let store = RedbStore::create(&path).expect("создано");
    put(&store, &record(6, EnrollmentStatus::Active));

    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Deny,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    // ⚠️ “Policy banned”  ⁇  “no registrations”.
    assert_eq!(
        result.err(),
        Some(AcceptError::Unauthorized(AuthorityRefusal::Denied))
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn pending_reconciliation_blocks_the_whole_request() {
    let path = temp_path("pending");
    let store = RedbStore::create(&path).expect("создано");
    put(&store, &record(7, EnrollmentStatus::Active));
    put(&store, &record(8, EnrollmentStatus::PendingReconciliation));

    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(matches!(result.err(), Some(AcceptError::Scope(_))));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn scope_lock_blocks_active_enrollment() {
    let path = temp_path("scope-lock");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record(7, EnrollmentStatus::Active));
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::MutationScopeLock,
        &scope_of(subject(), authenticator()),
        &[9; 16],
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(matches!(
        result,
        Err(AcceptError::Scope(
            carosign_runtime::authority::SelectionError::PendingReconciliation
        ))
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn blocked_scope_does_not_block_another_subject() {
    let path = temp_path("independent-scope");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record(7, EnrollmentStatus::Active));
    let mut other = record(8, EnrollmentStatus::Active);
    other.subject.subject = SubjectId::from_bytes([8; 16]).unwrap();
    let mut tx = store.begin_write().unwrap();
    let bytes = carosign_persistence::enrollment_codec::encode_v3(&other).unwrap();
    tx.put(Table::Enrollments, other.enrollment.id.as_bytes(), &bytes)
        .unwrap();
    let mut index = scope_of(other.subject, authenticator()).to_vec();
    index.extend_from_slice(other.enrollment.id.as_bytes());
    tx.put(
        Table::EnrollmentIndex,
        &index,
        other.enrollment.id.as_bytes(),
    )
    .unwrap();
    tx.put(
        Table::MutationScopeLock,
        &scope_of(subject(), authenticator()),
        &[9; 16],
    )
    .unwrap();
    tx.commit_durable().unwrap();

    let check = |subject| {
        futures_executor::block_on(prepare_verification(
            &store,
            &Allow,
            &caller(),
            subject,
            authenticator(),
            DeviceEpoch::FIRST,
        ))
    };
    assert!(matches!(check(subject()), Err(AcceptError::Scope(_))));
    assert!(check(other.subject).is_ok());
    let _ = std::fs::remove_file(path);
}

#[test]
fn scope_locked_during_authorization_is_refused() {
    let path = temp_path("lock-during-authorization");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record(7, EnrollmentStatus::Active));
    let result = futures_executor::block_on(prepare_verification(
        &store,
        &LockDuringAuthorization(&store),
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(matches!(
        result,
        Err(AcceptError::Scope(
            carosign_runtime::authority::SelectionError::PendingReconciliation
        ))
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn completed_change_during_authorization_refreshes_generation() {
    let path = temp_path("change-during-authorization");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record(7, EnrollmentStatus::Active));
    let cycle = futures_executor::block_on(prepare_verification(
        &store,
        &ChangeDuringAuthorization(&store),
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ))
    .unwrap();
    let carosign_runtime::ports::CommandPlan::Verify { permitted } = cycle.plan() else {
        panic!("ожидался план проверки")
    };
    assert_eq!(permitted.bindings()[0].enrollment.generation.value(), 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn dangling_or_mismatched_index_is_corruption_not_empty_or_another_binding() {
    let path = temp_path("bad-index");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record(7, EnrollmentStatus::Active));
    let mut index = [0u8; 64];
    index[..48].copy_from_slice(&scope_of(subject(), authenticator()));
    index[48..].copy_from_slice(&[8; 16]);
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::EnrollmentIndex, &index, &[8; 16]).unwrap();
    tx.commit_durable().unwrap();
    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(matches!(result, Err(AcceptError::IdentityUnavailable)));
    let mut tx = store.begin_write().unwrap();
    tx.put(Table::EnrollmentIndex, &index, &[7; 16]).unwrap();
    tx.commit_durable().unwrap();
    let result = futures_executor::block_on(prepare_verification(
        &store,
        &Allow,
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(matches!(result, Err(AcceptError::IdentityUnavailable)));
    let _ = std::fs::remove_file(path);
}

#[test]
fn scope_matches_the_registry_index() {
    let scope = scope_of(subject(), authenticator());
    assert_eq!(scope.len(), 48);
}

#[test]
fn answer_refuses_with_its_own_code() {
    use carosign_protocol::carosign::common::v1 as common;
    use carosignd::answer::{Reply, error_of};

    // ⚠️ Three troubles are three wire codes, not one "failure."
    assert_eq!(
        error_of(&AcceptError::NotEnrolled).code,
        common::ErrorCode::ErrorSubjectUnknown as i32
    );
    assert_eq!(
        error_of(&AcceptError::IdentityUnavailable).code,
        common::ErrorCode::ErrorUnavailable as i32
    );

    let reply = Reply::Refused(error_of(&AcceptError::NotEnrolled));
    let frame = reply.frame(&[1u8; 16]);
    // ⚠️ `request_id` Echo: without it, the answer cannot be linked to the request.
    assert_eq!(frame.request_id, vec![1u8; 16]);
}
