use super::*;
use carosign_engine::tests_support::initial;

#[test]
fn grant_must_be_issued_by_owner_authority() {
    let actor = OperationActor::new(initial());
    let context = &actor.state().common.context;
    let grant = |issuer| {
        ValidatedGrant::issue_for_port(
            context.owner.binding,
            issuer,
            actor.state().required_grant_scope(),
            context.deadline.boot(),
            context.deadline.instant_ns(),
        )
    };
    assert!(grant_matches_operation(
        &actor,
        &grant(context.owner.authority)
    ));
    let foreign = carosign_core::ids::AuthorityId::from_bytes([0xff; 16]).unwrap();
    assert_ne!(foreign, context.owner.authority);
    assert!(!grant_matches_operation(&actor, &grant(foreign)));
}

#[test]
fn new_operation_needs_three_steps_to_send() {
    let actor = OperationActor::new(initial());
    assert_eq!(steps_to_send(&actor), Some(3));
    assert!(!waits_for_provider(OperationPhase::New));
}

#[test]
fn running_waits_for_the_provider() {
    assert!(waits_for_provider(OperationPhase::Running));
    assert!(waits_for_provider(OperationPhase::InFlight));
    assert!(!waits_for_provider(OperationPhase::Finalizing));
}

#[test]
fn no_pending_effect_at_start() {
    let actor = OperationActor::new(initial());
    assert_eq!(next_effect(&actor), None);
}
