#[path = "policy_authority_tests_fixtures/armed_revoking_clock.rs"]
mod armed_revoking_clock;
#[path = "policy_authority_tests_fixtures/final_read_revoking_clock.rs"]
mod final_read_revoking_clock;
#[path = "policy_authority_tests_fixtures/regressing_clock.rs"]
mod regressing_clock;

use super::*;
use carosign_adapters::clock::StepClock;
use carosign_adapters::policy::subject_of;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, BootId};
use carosign_core::operation::context::VerificationPurpose;
use carosign_runtime::authority::verification::PurposeHint;
use carosign_runtime::ports::TransactionId;
use futures_executor::block_on;

#[test]
fn named_account_requires_trusted_caller_and_explicit_policy() {
    let _lock = crate::policy_test_lock();
    let dir = Path::new("/dev/shm").join(format!("carosign-policy-account-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    let text = "uid=0 subject=0 actions=verify purpose=session-login\n\
                    uid=0 subject=1000 actions=verify purpose=session-login\n\
                    uid=1000 subject=1000 actions=verify purpose=session-unlock\n\
                    uid=1001 subject=1000 actions=verify purpose=session-login\n";
    std::fs::write(&path, text).unwrap();
    let authority = AuthorityId::from_bytes([1; 16]).unwrap();
    let source = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock: Arc::new(StepClock::new(BootId::from_bytes([2; 16]).unwrap())),
        state: Mutex::new(State {
            watch: PolicyChangeWatch::new(&path).unwrap(),
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    let caller = |uid| {
        CallerContext::from_transport([1; 32], uid, TransactionId::from_bytes([3; 16]).unwrap())
    };
    let expected = SubjectRef {
        authority,
        subject: subject_of(1000),
    };
    // sudo: A privileged PAM process verifies the calling UID 1000.
    assert_eq!(
        source.resolve_account_uid(&caller(0), 1000).unwrap(),
        expected
    );
    // su: The target root user is verified, regardless of the caller’s UID.
    assert_eq!(
        source.resolve_account_uid(&caller(0), 0).unwrap().subject,
        subject_of(0)
    );
    assert!(matches!(
        source.resolve_account_uid(&caller(1000), 0),
        Err(IdentityError::NotPermitted)
    ));
    assert_eq!(
        source.resolve_account_uid(&caller(1000), 1000).unwrap(),
        expected
    );
    assert!(matches!(
        source.resolve_account_uid(&caller(1001), 1000),
        Err(IdentityError::NotPermitted)
    ));
    assert!(matches!(
        source.resolve_account_uid(&caller(0), 1001),
        Err(IdentityError::NotPermitted)
    ));
    assert!(matches!(
        account_uid("bad\0name"),
        Err(IdentityError::UnknownSubject)
    ));
    if account_uid("root").ok() == Some(0) {
        let intent = VerificationIntent {
            report_matched_enrollment: false,
            enrollment: None,
            subject: RequestedSubject::AccountHint("root".to_string()),
            purpose_hint: PurposeHint::SessionLogin,
            authority_request_ref: Vec::new(),
            authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        };
        assert_eq!(
            block_on(source.resolve_verification(&caller(0), &intent)).unwrap(),
            SubjectRef {
                authority,
                subject: subject_of(0)
            },
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn observed_expiry_cannot_be_undone_by_clock_regression() {
    use std::sync::atomic::{AtomicU64, Ordering};

    use regressing_clock::RegressingClock;

    let _lock = crate::policy_test_lock();
    let dir = Path::new("/dev/shm").join(format!("carosign-policy-expiry-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    let text = "uid=1000 subject=42 actions=verify purpose=session-login lifetime=30\n";
    std::fs::write(&path, text).unwrap();
    let authority = AuthorityId::from_bytes([1; 16]).unwrap();
    let clock = Arc::new(RegressingClock {
        boot: BootId::from_bytes([2; 16]).unwrap(),
        nanos: AtomicU64::new(0),
    });
    let source = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock: clock.clone(),
        state: Mutex::new(State {
            watch: PolicyChangeWatch::new(&path).unwrap(),
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([3; 16]).unwrap());
    let scope = GrantScope::Verify {
        subject: SubjectRef {
            authority,
            subject: subject_of(42),
        },
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        purpose: VerificationPurpose::SessionLogin,
    };
    let grant = block_on(source.authorize(&caller, scope)).unwrap();
    clock.nanos.store(grant.deadline_ns(), Ordering::SeqCst);
    assert_eq!(
        source.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Expired)
    );
    clock.nanos.store(0, Ordering::SeqCst);
    assert_eq!(
        source.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn issuance_reclaims_expired_grants_without_reclaiming_live_ones() {
    let _lock = crate::policy_test_lock();
    let dir = Path::new("/dev/shm").join(format!("carosign-policy-prune-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    let text = "uid=1000 subject=42 actions=verify purpose=session-login lifetime=30\n";
    std::fs::write(&path, text).unwrap();
    let authority = AuthorityId::from_bytes([1; 16]).unwrap();
    let clock = Arc::new(StepClock::new(BootId::from_bytes([2; 16]).unwrap()));
    let source = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock: clock.clone(),
        state: Mutex::new(State {
            watch: PolicyChangeWatch::new(&path).unwrap(),
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([3; 16]).unwrap());
    let scope = GrantScope::Verify {
        subject: SubjectRef {
            authority,
            subject: subject_of(42),
        },
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        purpose: VerificationPurpose::SessionLogin,
    };
    let expired = block_on(source.authorize(&caller, scope.clone())).unwrap();
    clock.advance(15_000_000_000);
    let live = block_on(source.authorize(&caller, scope.clone())).unwrap();
    clock.advance(15_000_000_000);
    let fresh = block_on(source.authorize(&caller, scope)).unwrap();
    let state = source.state.lock().unwrap();
    assert!(!state.issued.contains_key(&expired.id()));
    assert!(state.issued.contains_key(&live.id()));
    assert!(state.issued.contains_key(&fresh.id()));
    drop(state);
    assert!(source.revalidate_for(&caller, &live).is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn issuance_fails_if_policy_changes_during_final_clock_read() {
    use std::sync::atomic::AtomicUsize;

    use final_read_revoking_clock::RevokingClock;

    let _lock = crate::policy_test_lock();
    let dir = Path::new("/dev/shm").join(format!("carosign-policy-issue-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    let text = "uid=1000 subject=42 actions=verify purpose=session-login lifetime=30\n";
    std::fs::write(&path, text).unwrap();
    let authority = AuthorityId::from_bytes([1; 16]).unwrap();
    let clock = Arc::new(RevokingClock {
        boot: BootId::from_bytes([2; 16]).unwrap(),
        path: path.clone(),
        reads: AtomicUsize::new(0),
    });
    let source = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock,
        state: Mutex::new(State {
            watch: PolicyChangeWatch::new(&path).unwrap(),
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([3; 16]).unwrap());
    let scope = GrantScope::Verify {
        subject: SubjectRef {
            authority,
            subject: subject_of(42),
        },
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        purpose: VerificationPurpose::SessionLogin,
    };
    assert!(matches!(
        block_on(source.authorize(&caller, scope)),
        Err(AuthorityRefusal::Revoked)
    ));
    let state = source.state.lock().unwrap();
    assert!(state.invalid);
    assert!(state.issued.is_empty());
    drop(state);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn revocation_during_clock_read_is_not_reported_as_valid() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use armed_revoking_clock::RevokingClock;

    let _lock = crate::policy_test_lock();
    let dir = Path::new("/dev/shm").join(format!("carosign-policy-clock-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    let text = "uid=1000 subject=42 actions=verify purpose=session-login lifetime=30\n";
    std::fs::write(&path, text).unwrap();
    let clock = Arc::new(RevokingClock {
        boot: BootId::from_bytes([2; 16]).unwrap(),
        path: path.clone(),
        armed: AtomicBool::new(false),
    });
    let authority = AuthorityId::from_bytes([1; 16]).unwrap();
    let source = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock: clock.clone(),
        state: Mutex::new(State {
            watch: PolicyChangeWatch::new(&path).unwrap(),
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([3; 16]).unwrap());
    let scope = GrantScope::Verify {
        subject: SubjectRef {
            authority,
            subject: subject_of(42),
        },
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        purpose: VerificationPurpose::SessionLogin,
    };
    let grant = block_on(source.authorize(&caller, scope)).unwrap();
    clock.armed.store(true, Ordering::SeqCst);
    assert_eq!(
        source.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    assert_eq!(
        source.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn issued_grants_bind_caller_and_are_revoked_on_policy_change() {
    let _lock = crate::policy_test_lock();
    let directory = std::path::Path::new("/dev/shm")
        .join(format!("carosign-policy-owner-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("policy");
    let text = "uid=1000 subject=42 actions=verify purpose=session-login lifetime=30\n";
    std::fs::write(&path, text).unwrap();
    // The temporary directory is not a trusted system source: only ownership and recall are
    // verified after issuance.
    let watch = PolicyChangeWatch::new(&path).unwrap();
    let authority = AuthorityId::from_bytes([1; 16]).unwrap();
    let clock = Arc::new(StepClock::new(BootId::from_bytes([2; 16]).unwrap()));
    let source = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock,
        state: Mutex::new(State {
            watch,
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([3; 16]).unwrap());
    let other =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([4; 16]).unwrap());
    let resolved = block_on(source.resolve(&caller, &SubjectSelector::Current)).unwrap();
    assert_eq!(resolved.authority, authority);
    assert_eq!(resolved.subject, subject_of(42));
    let mut intent = VerificationIntent {
        report_matched_enrollment: false,
        enrollment: None,
        subject: RequestedSubject::Current,
        purpose_hint: PurposeHint::SessionLogin,
        authority_request_ref: Vec::new(),
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
    };
    assert_eq!(
        block_on(source.resolve_verification(&caller, &intent)),
        Ok(resolved)
    );
    let verified = block_on(source.authorize_verification(&caller, &intent, resolved)).unwrap();
    assert!(matches!(
        verified.scope(),
        GrantScope::Verify {
            purpose: VerificationPurpose::SessionLogin,
            ..
        }
    ));
    intent.purpose_hint = PurposeHint::KeyOperation;
    let same_policy_target =
        block_on(source.authorize_verification(&caller, &intent, resolved)).unwrap();
    assert!(matches!(
        same_policy_target.scope(),
        GrantScope::Verify {
            purpose: VerificationPurpose::SessionLogin,
            ..
        }
    ));
    intent.authority_request_ref = vec![1];
    assert!(matches!(
        block_on(source.authorize_verification(&caller, &intent, resolved)),
        Err(AuthorityRefusal::Denied)
    ));
    intent.authority_request_ref.clear();
    let foreign_subject = SubjectRef {
        authority,
        subject: subject_of(43),
    };
    assert!(matches!(
        block_on(source.authorize_verification(&caller, &intent, foreign_subject)),
        Err(AuthorityRefusal::Denied)
    ));
    intent.subject = RequestedSubject::AccountHint("другой".into());
    assert_eq!(
        block_on(source.resolve_verification(&caller, &intent)),
        Err(IdentityError::UnknownSubject)
    );
    let scope = GrantScope::Verify {
        subject: SubjectRef {
            authority,
            subject: subject_of(42),
        },
        authenticator: AuthenticatorId::from_bytes([5; 16]).unwrap(),
        purpose: VerificationPurpose::SessionLogin,
    };
    let mut wrong_scope = scope.clone();
    if let GrantScope::Verify { purpose, .. } = &mut wrong_scope {
        *purpose = VerificationPurpose::KeyOperation;
    }
    assert!(matches!(
        block_on(source.authorize(&caller, wrong_scope)),
        Err(AuthorityRefusal::Denied)
    ));
    let grant = block_on(source.authorize(&caller, scope)).unwrap();
    // Even the same content does not transfer the issued rights to another copy of the owner
    // of the image: a new issue checks a new table.
    let other_snapshot = SnapshotAuthority {
        policy: policy::parse(text).unwrap(),
        path: path.clone(),
        authority,
        clock: source.clock.clone(),
        state: Mutex::new(State {
            watch: PolicyChangeWatch::new(&path).unwrap(),
            issued: HashMap::new(),
            invalid: false,
        }),
    };
    assert_eq!(
        other_snapshot.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    assert_eq!(
        block_on(source.revalidate(&grant, AuthorizationBoundary::BeforeDelivery)),
        Err(AuthorityRefusal::Unavailable)
    );
    assert!(matches!(
        source.operation_watch_for(&other, &grant),
        Err(AuthorityRefusal::Revoked)
    ));
    let mut operation_watch = source.operation_watch_for(&caller, &grant).unwrap();
    let mut second_watch = source.operation_watch_for(&caller, &grant).unwrap();
    assert!(matches!(
        operation_watch.check(),
        Ok(PolicyWatchState::Unchanged)
    ));
    assert!(source.revalidate_for(&caller, &grant).is_ok());
    assert!(
        block_on(source.revalidate_caller(&caller, &grant, AuthorizationBoundary::BeforeDelivery))
            .is_ok()
    );
    assert_eq!(
        block_on(source.revalidate_caller(&other, &grant, AuthorizationBoundary::BeforeDelivery)),
        Err(AuthorityRefusal::Revoked)
    );
    assert_eq!(
        source.revalidate_for(&other, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    let forged = ValidatedGrant::issue_for_port(
        grant.id(),
        grant.issuer(),
        grant.scope().clone(),
        grant.boot(),
        grant.deadline_ns() + 1,
    );
    assert_eq!(
        block_on(source.revalidate(&forged, AuthorizationBoundary::BeforeDelivery)),
        Err(AuthorityRefusal::Unavailable)
    );
    std::fs::write(&path, "# revoked\n").unwrap();
    // Recheck grant and operations monitors have independent queues: reading the event by the
    // owner of the picture should not hide the feedback from operations.
    assert_eq!(
        source.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    assert!(matches!(
        operation_watch.check(),
        Ok(PolicyWatchState::Invalidated)
    ));
    assert!(matches!(
        second_watch.check(),
        Ok(PolicyWatchState::Invalidated)
    ));
    assert!(matches!(
        source.operation_watch(),
        Err(AuthorityRefusal::Revoked)
    ));
    assert!(matches!(
        source.operation_watch_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    ));
    assert_eq!(
        block_on(source.resolve(&caller, &SubjectSelector::Current)),
        Err(IdentityError::Unavailable)
    );
    assert_eq!(
        source.revalidate_for(&caller, &grant),
        Err(AuthorityRefusal::Revoked)
    );
    assert_eq!(
        block_on(source.revalidate(&grant, AuthorizationBoundary::BeforeDelivery)),
        Err(AuthorityRefusal::Unavailable)
    );
    std::fs::remove_dir_all(directory).unwrap();
}
