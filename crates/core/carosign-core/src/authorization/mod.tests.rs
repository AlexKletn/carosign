use crate::enrollment::EnrollmentRef;
use crate::identity::SubjectRef;
use crate::ids::AuthenticatorId;
use crate::ids::BootId;
use crate::ids::OperationId;
use crate::operation::context::VerificationPurpose;

use super::*;
use crate::ids::{AuthorityId, GrantId, SubjectId};

fn ids() -> (GrantId, AuthorityId, BootId) {
    (
        GrantId::from_bytes([1u8; 16]).expect("валид"),
        AuthorityId::from_bytes([2u8; 16]).expect("валид"),
        BootId::from_bytes([3u8; 16]).expect("валид"),
    )
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([4u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([5u8; 16]).expect("валид"),
    }
}

fn verify() -> GrantScope {
    GrantScope::Verify {
        subject: subject(),
        authenticator: AuthenticatorId::from_bytes([7; 16]).unwrap(),
        purpose: VerificationPurpose::SessionLogin,
    }
}

fn enroll() -> GrantScope {
    GrantScope::Enroll {
        subject: subject(),
        authenticator: AuthenticatorId::from_bytes([7; 16]).unwrap(),
        replace: None,
    }
}

#[test]
fn grant_covers_only_its_subject() {
    let (id, issuer, boot) = ids();
    let grant = ValidatedGrant::issue_for_port(id, issuer, enroll(), boot, 1_000);
    assert!(grant.covers(subject()));
    let other = SubjectRef {
        authority: subject().authority,
        subject: SubjectId::from_bytes([6u8; 16]).expect("валид"),
    };
    assert!(!grant.covers(other));
}

#[test]
fn operation_scopes_are_bound_to_one_operation() {
    let operation = OperationId::from_bytes([8; 16]).unwrap();
    let other = OperationId::from_bytes([9; 16]).unwrap();
    assert_eq!(GrantScope::Observe(operation).operation(), Some(operation));
    assert_ne!(GrantScope::Observe(operation), GrantScope::Observe(other));
    assert_ne!(
        GrantScope::Observe(operation),
        GrantScope::Cancel(operation)
    );
    assert_eq!(GrantScope::Observe(operation).subject(), None);
    assert_eq!(GrantScope::Cancel(operation).subject(), None);
    assert!(GrantScope::ManageProvider.subject().is_none());
    assert!(GrantScope::ManageProvider.operation_kind().is_none());
    assert!(enroll().is_mutating());
    assert!(!verify().is_mutating());
}

#[test]
fn grant_ref_carries_no_scope() {
    let (id, issuer, boot) = ids();
    let grant = ValidatedGrant::issue_for_port(id, issuer, verify(), boot, 5);
    let reference = ScopedGrantRef::of(&grant);
    assert_eq!(reference.id(), id);
    assert_eq!(reference.issuer(), issuer);
    // The link does not contain a domain; you need to ask it again.
    assert_eq!(format!("{reference:?}"), "ScopedGrantRef(..)");
}

#[test]
fn grant_scope_distinguishes_device_purpose_and_enrollment_generation() {
    use crate::enrollment::EnrollmentGeneration;
    use crate::ids::EnrollmentId;
    let authenticator = AuthenticatorId::from_bytes([7; 16]).unwrap();
    let other_authenticator = AuthenticatorId::from_bytes([8; 16]).unwrap();
    let verification = verify();
    assert_ne!(
        verification,
        GrantScope::Verify {
            subject: subject(),
            authenticator: other_authenticator,
            purpose: VerificationPurpose::SessionLogin
        }
    );
    assert_ne!(
        verification,
        GrantScope::Verify {
            subject: subject(),
            authenticator,
            purpose: VerificationPurpose::KeyOperation
        }
    );
    let old = EnrollmentRef {
        id: EnrollmentId::from_bytes([9; 16]).unwrap(),
        authenticator,
        generation: EnrollmentGeneration::FIRST,
    };
    let newer = EnrollmentRef {
        generation: old.generation.next().unwrap(),
        ..old
    };
    assert_ne!(
        GrantScope::Enroll {
            subject: subject(),
            authenticator,
            replace: Some(old)
        },
        GrantScope::Enroll {
            subject: subject(),
            authenticator,
            replace: Some(newer)
        }
    );
    assert_ne!(
        GrantScope::Remove {
            subject: subject(),
            enrollment: old
        },
        GrantScope::Remove {
            subject: subject(),
            enrollment: newer
        }
    );
}
