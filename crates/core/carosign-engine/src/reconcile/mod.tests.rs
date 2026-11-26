use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::ids::EnrollmentId;
use carosign_core::ids::EvidenceId;

use super::*;

fn id(byte: u8) -> EnrollmentId {
    EnrollmentId::from_bytes([byte; 16]).expect("валид")
}

fn state(byte: u8) -> VerifiedState {
    VerifiedState {
        enrollment: id(byte),
        generation: EnrollmentGeneration::FIRST,
        provenance: EvidenceId::from_bytes([9u8; 16]).expect("валид"),
    }
}

fn expected() -> ExpectedStates {
    // Original - old registration 1, desired - new 2 (replacement).
    ExpectedStates {
        pre: id(1),
        desired: id(2),
    }
}

#[test]
fn matching_desired_id_is_not_a_complete_observation() {
    let verdict = evaluate(expected(), Some(state(2)), None);
    assert_eq!(
        verdict,
        ReconciliationVerdict::Indeterminate(ReconciliationFailure::Unknown)
    );
    assert!(!verdict.allows_commit());
}

#[test]
fn matching_pre_id_is_not_a_complete_observation() {
    let verdict = evaluate(expected(), Some(state(1)), None);
    assert_eq!(
        verdict,
        ReconciliationVerdict::Indeterminate(ReconciliationFailure::Unknown)
    );
    assert!(!verdict.allows_commit());
}

#[test]
fn missing_new_template_is_not_no_change_when_old_vanished() {
    // B-01: "Nothing new" when replaced - "nothing has changed": the old registration is
    // gone, and the condition does not match either the original or the desired.
    let verdict = evaluate(expected(), Some(state(7)), None);
    assert_eq!(
        verdict,
        ReconciliationVerdict::Indeterminate(ReconciliationFailure::Unknown)
    );
    assert!(!verdict.allows_commit());
}

#[test]
fn indeterminate_blocks_commit() {
    let verdict = evaluate(expected(), None, Some(ReconciliationFailure::Unsupported));
    assert!(!verdict.allows_commit());
    assert_eq!(verdict.registry_action(), None);
}

#[test]
fn no_observation_is_unknown() {
    assert_eq!(
        evaluate(expected(), None, None),
        ReconciliationVerdict::Indeterminate(ReconciliationFailure::Unknown)
    );
}
