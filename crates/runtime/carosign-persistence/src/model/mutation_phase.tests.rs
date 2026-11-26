use super::*;

#[test]
fn dispatch_only_from_armed() {
    assert!(MutationPhase::DispatchArmed.allows_dispatch());
    assert!(!MutationPhase::Prepared.allows_dispatch());
    assert!(!MutationPhase::Completed.allows_dispatch());
}

#[test]
fn armed_may_have_touched_hardware() {
    assert!(MutationPhase::DispatchArmed.may_have_touched_hardware());
    assert!(!MutationPhase::Prepared.may_have_touched_hardware());
}
