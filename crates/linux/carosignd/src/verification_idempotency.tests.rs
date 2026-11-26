use super::*;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, AuthorityId, SubjectId};

fn setup() -> (VerificationIntent, GrantScope) {
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let intent = VerificationIntent {
        report_matched_enrollment: false,
        enrollment: None,
        subject: RequestedSubject::AccountHint("alice".into()),
        purpose_hint: PurposeHint::SessionLogin,
        authority_request_ref: vec![4, 5],
        authenticator,
    };
    let scope = GrantScope::Verify {
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([1; 16]).unwrap(),
            subject: SubjectId::from_bytes([2; 16]).unwrap(),
        },
        authenticator,
        purpose: VerificationPurpose::SessionUnlock,
    };
    (intent, scope)
}

#[test]
fn each_request_field_and_policy_scope_affects_digest() {
    let (intent, scope) = setup();
    let original = RequestDigest::new(&intent, &scope).unwrap();
    assert!(original == RequestDigest::new(&intent, &scope).unwrap());
    let mut altered = intent.clone();
    altered.subject = RequestedSubject::Opaque(b"alice".to_vec());
    assert!(original != RequestDigest::new(&altered, &scope).unwrap());
    altered = intent.clone();
    altered.purpose_hint = PurposeHint::SessionUnlock;
    assert!(original != RequestDigest::new(&altered, &scope).unwrap());
    altered = intent.clone();
    altered.authority_request_ref.push(0);
    assert!(original != RequestDigest::new(&altered, &scope).unwrap());
    altered = intent.clone();
    altered.report_matched_enrollment = true;
    assert!(original != RequestDigest::new(&altered, &scope).unwrap());
    altered = intent.clone();
    altered.enrollment = Some(carosign_core::enrollment::EnrollmentRef {
        id: carosign_core::ids::EnrollmentId::from_bytes([10; 16]).unwrap(),
        authenticator: intent.authenticator,
        generation: carosign_core::enrollment::EnrollmentGeneration::new(1).unwrap(),
    });
    let selected = RequestDigest::new(&altered, &scope).unwrap();
    assert!(original != selected);
    altered.enrollment.as_mut().unwrap().generation =
        carosign_core::enrollment::EnrollmentGeneration::new(2).unwrap();
    assert!(selected != RequestDigest::new(&altered, &scope).unwrap());
    let GrantScope::Verify {
        subject,
        authenticator,
        ..
    } = scope
    else {
        unreachable!()
    };
    let other = GrantScope::Verify {
        subject,
        authenticator,
        purpose: VerificationPurpose::SessionLogin,
    };
    assert!(original != RequestDigest::new(&intent, &other).unwrap());
    let other = GrantScope::Verify {
        subject: SubjectRef {
            authority: subject.authority,
            subject: SubjectId::from_bytes([8; 16]).unwrap(),
        },
        authenticator,
        purpose: VerificationPurpose::SessionUnlock,
    };
    assert!(original != RequestDigest::new(&intent, &other).unwrap());
    altered = intent.clone();
    altered.authenticator = AuthenticatorId::from_bytes([9; 16]).unwrap();
    assert!(RequestDigest::new(&altered, &other).is_err());
}
