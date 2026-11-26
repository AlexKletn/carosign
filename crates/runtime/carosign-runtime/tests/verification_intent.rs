//! Verification of the transfer of customer intent and limits of trust.
#[path = "verification_intent_fixtures/authority.rs"]
mod authority;
#[path = "verification_intent_fixtures/legacy.rs"]
mod legacy;

use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::identity::{IdentityError, SubjectRef, SubjectSelector};
use carosign_core::ids::{AuthenticatorId, AuthorityId, BootId, GrantId, SubjectId};
use carosign_core::operation::context::VerificationPurpose;
use carosign_protocol::carosign::common::v1::{
    SubjectSelector as WireSelector, subject_selector::Selector,
};
use carosign_runtime::authority::verification::{
    PurposeHint, RequestedSubject, VerificationAuthorityError, VerificationIntent,
    authorize_verification,
};
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, IdentityAuthority, PolicyAuthority, TransactionId,
};
use futures_executor::block_on;

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    }
}
fn caller() -> CallerContext {
    CallerContext::from_transport([3; 32], 42, TransactionId::from_bytes([4; 16]).unwrap())
}
fn intent(selector: Selector, purpose: i32, reference: &[u8]) -> VerificationIntent {
    VerificationIntent::from_wire(
        &WireSelector {
            selector: Some(selector),
        },
        purpose,
        reference,
        AuthenticatorId::from_bytes([5; 16]).unwrap(),
    )
    .unwrap()
}

use authority::Authority;

#[test]
fn all_selectors_and_purposes_reach_both_authorities() {
    for selector in [
        Selector::CurrentSubject(true),
        Selector::AccountHint("имя".into()),
        Selector::OpaqueAuthoritySelector(vec![0, 255, 3]),
    ] {
        for purpose in 1..=5 {
            let request = intent(selector.clone(), purpose, &[0, 255, 17]);
            let authority = Authority {
                expected: request.clone(),
                wrong_scope: false,
                revoked: false,
            };
            assert!(
                block_on(authorize_verification(
                    &authority,
                    &authority,
                    &caller(),
                    &request
                ))
                .is_ok()
            );
            assert!(match (&request.subject, &selector) {
                (RequestedSubject::Current, Selector::CurrentSubject(true)) => true,
                (RequestedSubject::AccountHint(a), Selector::AccountHint(b)) => a == b,
                (RequestedSubject::Opaque(a), Selector::OpaqueAuthoritySelector(b)) => a == b,
                _ => false,
            });
            assert!(matches!(
                (purpose, request.purpose_hint),
                (1, PurposeHint::SessionLogin)
                    | (2, PurposeHint::SessionUnlock)
                    | (3, PurposeHint::PrivilegedAction)
                    | (4, PurposeHint::SelfCheck)
                    | (5, PurposeHint::KeyOperation)
            ));
            assert_eq!(request.authority_request_ref, [0, 255, 17]);
        }
    }
}

#[test]
fn malformed_input_wrong_scope_and_revocation_are_rejected() {
    let id = AuthenticatorId::from_bytes([5; 16]).unwrap();
    for selector in [
        None,
        Some(Selector::CurrentSubject(false)),
        Some(Selector::OpaqueAuthoritySelector(vec![])),
    ] {
        assert!(VerificationIntent::from_wire(&WireSelector { selector }, 1, &[], id).is_err());
    }
    let valid = WireSelector {
        selector: Some(Selector::CurrentSubject(true)),
    };
    assert!(VerificationIntent::from_wire(&valid, 0, &[], id).is_err());
    assert!(VerificationIntent::from_wire(&valid, 1, &[1; 257], id).is_err());
    let request = intent(Selector::CurrentSubject(true), 1, &[]);
    let authority = Authority {
        expected: request.clone(),
        wrong_scope: true,
        revoked: false,
    };
    assert!(matches!(
        block_on(authorize_verification(
            &authority,
            &authority,
            &caller(),
            &request
        )),
        Err(VerificationAuthorityError::ScopeMismatch)
    ));
    let authority = Authority {
        expected: request.clone(),
        wrong_scope: false,
        revoked: true,
    };
    assert!(matches!(
        block_on(authorize_verification(
            &authority,
            &authority,
            &caller(),
            &request
        )),
        Err(VerificationAuthorityError::Policy(
            AuthorityRefusal::Revoked
        ))
    ));
}

#[test]
fn authority_without_caller_binding_cannot_create_verification() {
    use legacy::Legacy;

    let request = intent(Selector::CurrentSubject(true), 1, &[]);
    let identity = Authority {
        expected: request.clone(),
        wrong_scope: false,
        revoked: false,
    };
    let legacy = Legacy(Authority {
        expected: request.clone(),
        wrong_scope: false,
        revoked: false,
    });
    assert!(matches!(
        block_on(authorize_verification(
            &identity,
            &legacy,
            &caller(),
            &request
        )),
        Err(VerificationAuthorityError::Policy(
            AuthorityRefusal::Unavailable
        ))
    ));
}
