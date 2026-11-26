use super::*;

#[test]
fn generation_must_match_exactly() {
    assert!(generation_matches(3, 3));
    assert!(!generation_matches(3, 4));
}

#[test]
fn removal_confirms_by_both_state_and_generation() {
    assert_eq!(
        removal_registry_action(true),
        crate::reconcile::RegistryAction::ApplyDesiredState
    );
    assert_eq!(
        removal_registry_action(false),
        crate::reconcile::RegistryAction::RecordNoChange
    );
}
