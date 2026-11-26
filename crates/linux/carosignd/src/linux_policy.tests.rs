use super::*;
use carosign_adapters::clock::StepClock;
use carosign_core::ids::BootId;

#[test]
fn subject_uid_is_resolved_by_the_platform_and_rejects_foreign_authorities() {
    use carosign_core::ids::{AuthorityId, SubjectId};
    use carosign_protocol::carosign::common::v1::{SubjectSelector, subject_selector::Selector};
    use carosign_runtime::ports::TransactionId;

    let path = std::env::temp_dir().join(format!("carosign-linux-uid-{}", std::process::id()));
    std::fs::write(
        &path,
        "uid=0 subject=0 actions=verify purpose=session-login\n",
    )
    .unwrap();
    let clock = Arc::new(StepClock::new(BootId::from_bytes([1; 16]).unwrap()));
    let policy = LinuxPolicy::fixture(&path, clock);
    let root =
        CallerContext::from_transport([1; 32], 0, TransactionId::from_bytes([2; 16]).unwrap());
    let user =
        CallerContext::from_transport([3; 32], 1000, TransactionId::from_bytes([4; 16]).unwrap());
    let root_selector = SubjectSelector {
        selector: Some(Selector::AccountHint("root".into())),
    };

    // su: root is the subject being verified, not the value coming from the client.
    let root_subject = policy
        .resolve_mutation_subject(&root, &root_selector)
        .unwrap();
    assert_eq!(policy.uid_of_subject(root_subject), Ok(0));
    assert!(matches!(
        policy.resolve_mutation_subject(&user, &root_selector),
        Err(IdentityError::NotPermitted)
    ));
    // sudo: The UID in the frame is taken from the subject, not the UID root process.
    let invoking_user = SubjectRef {
        authority: AuthorityId::from_bytes(*b"linux-local-v001").unwrap(),
        subject: carosign_adapters::policy::subject_of(1000),
    };
    assert_eq!(policy.uid_of_subject(invoking_user), Ok(1000));
    let foreign = SubjectRef {
        authority: AuthorityId::from_bytes([9; 16]).unwrap(),
        ..invoking_user
    };
    assert!(policy.uid_of_subject(foreign).is_err());
    let malformed = SubjectRef {
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
        ..invoking_user
    };
    assert!(policy.uid_of_subject(malformed).is_err());
    let overflow = SubjectRef {
        subject: carosign_adapters::policy::subject_of(u64::from(u32::MAX) + 1),
        ..invoking_user
    };
    assert!(policy.uid_of_subject(overflow).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn rejects_unprotected_policy_instead_of_silently_allowing() {
    let path = std::env::temp_dir().join(format!(
        "carosign-unprotected-policy-{}",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "uid=0 subject=0 actions=verify purpose=session-login\n",
    )
    .unwrap();
    let clock = Arc::new(StepClock::new(BootId::from_bytes([1; 16]).unwrap()));
    assert!(LinuxPolicy::open(&path, clock).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn inventory_requires_explicit_permission_and_rechecks_the_snapshot() {
    let path =
        std::env::temp_dir().join(format!("carosign-inventory-policy-{}", std::process::id()));
    std::fs::write(
        &path,
        "uid=0 subject=0 actions=observe\nuid=1000 subject=1000 actions=inventory\nuid=1001 subject=1000 actions=inventory\n",
    ).unwrap();
    let clock = Arc::new(StepClock::new(BootId::from_bytes([1; 16]).unwrap()));
    let policy = LinuxPolicy::fixture(&path, clock);
    assert_eq!(
        policy.0.authorize_inventory(0),
        Err(AuthorityRefusal::Denied)
    );
    assert_eq!(policy.0.authorize_inventory(1000), Ok(()));
    assert_eq!(
        policy.0.authorize_inventory(1001),
        Err(AuthorityRefusal::Denied)
    );
    std::fs::write(&path, "").unwrap();
    assert!(policy.0.authorize_inventory(1000).is_err());
    std::fs::remove_file(path).unwrap();
}
