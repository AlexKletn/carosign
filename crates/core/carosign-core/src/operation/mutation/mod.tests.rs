use crate::enrollment::EnrollmentRef;
use crate::ids::AuthenticatorId;
use crate::ids::MutationId;

use super::*;
use crate::enrollment::EnrollmentGeneration;
use crate::ids::EnrollmentId;

#[test]
fn reconciliation_cannot_be_reported_as_no_change() {
    let mutation = MutationId::from_bytes([1u8; 16]).expect("валид");
    let outcome = MutationOutcome::NeedsReconciliation(mutation);
    assert!(outcome.is_uncertain());
    assert!(!outcome.may_be_reported_as_no_change());
    assert!(MutationOutcome::NoChange.may_be_reported_as_no_change());
}

#[test]
fn committed_carries_generation() {
    let enrollment = EnrollmentRef {
        id: EnrollmentId::from_bytes([2u8; 16]).expect("валид"),
        authenticator: AuthenticatorId::from_bytes([3u8; 16]).expect("валид"),
        generation: EnrollmentGeneration::FIRST,
    };
    let outcome = MutationOutcome::Committed(enrollment);
    assert!(!outcome.is_uncertain());
    assert_eq!(outcome, MutationOutcome::Committed(enrollment));
}
