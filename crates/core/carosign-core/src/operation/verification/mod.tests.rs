use crate::enrollment::EnrollmentRef;
use crate::ids::AuthenticatorId;
use crate::ids::EvidenceId;

use super::*;
use crate::enrollment::EnrollmentGeneration;
use crate::ids::{AuthorityId, EnrollmentId, SubjectId};

#[test]
fn matched_is_never_user_success() {
    let evidence = EvidenceId::from_bytes([1u8; 16]).expect("валид");
    let outcome = VerificationOutcome::Matched(evidence);
    assert!(outcome.needs_evidence_check());
    assert!(!outcome.is_user_success());
    assert!(!VerificationOutcome::NotMatched.is_user_success());
}

#[test]
fn accepted_factor_is_bound_to_one_enrollment() {
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([1u8; 16]).expect("валид"),
        authenticator: AuthenticatorId::from_bytes([2u8; 16]).expect("валид"),
        generation: EnrollmentGeneration::FIRST,
    };
    let factor = AcceptedFactor::for_enrollment(enrollment);
    assert_eq!(factor.enrollment(), enrollment);
    // Appearance doesn't reveal content.
    assert_eq!(format!("{factor:?}"), "AcceptedFactor(..)");
    let _ = (
        AuthorityId::from_bytes([3u8; 16]).expect("валид"),
        SubjectId::from_bytes([4u8; 16]).expect("валид"),
    );
}
