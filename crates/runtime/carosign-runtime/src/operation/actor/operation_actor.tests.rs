use super::super::*;
use crate::operation::bindings::{BindingKind, EnrollmentBinding, PermittedSet};
use carosign_core::authorization::ValidatedGrant;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::ids::{AuthenticatorId, AuthorityId, EnrollmentId};
use carosign_engine::DomainInput;

#[test]
fn verification_bindings_are_fixed_before_admission_and_match_grant() {
    let state = carosign_engine::tests_support::initial();
    let context = &state.common.context;
    let grant = |issuer| {
        ValidatedGrant::issue_for_port(
            context.owner.binding,
            issuer,
            state.required_grant_scope(),
            context.deadline.boot(),
            context.deadline.instant_ns(),
        )
    };
    let binding = |authenticator| {
        PermittedSet::new(vec![EnrollmentBinding {
            enrollment: EnrollmentRef {
                id: EnrollmentId::from_bytes([12; 16]).unwrap(),
                authenticator,
                generation: EnrollmentGeneration::FIRST,
            },
            kind: BindingKind::ProviderOpaque,
            provider_ref: vec![1],
            format_id: None,
            format_version: 0,
        }])
        .unwrap()
    };
    let mut actor = OperationActor::new(state.clone());
    assert_eq!(
        actor.bind_verification_scope(
            &grant(AuthorityId::from_bytes([99; 16]).unwrap()),
            binding(context.authenticator.id)
        ),
        Err(ObservationError::ContextMismatch)
    );
    assert_eq!(
        actor.bind_verification_scope(
            &grant(context.owner.authority),
            binding(AuthenticatorId::from_bytes([88; 16]).unwrap())
        ),
        Err(ObservationError::ContextMismatch)
    );
    assert_eq!(
        actor.bind_verification_scope(
            &grant(context.owner.authority),
            binding(context.authenticator.id)
        ),
        Ok(())
    );
    assert_eq!(
        actor.bind_verification_scope(
            &grant(context.owner.authority),
            binding(context.authenticator.id)
        ),
        Err(ObservationError::AlreadyPending)
    );

    let mut late = OperationActor::new(state.clone());
    late.post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    assert_eq!(
        late.bind_verification_scope(
            &grant(context.owner.authority),
            binding(context.authenticator.id)
        ),
        Err(ObservationError::ContextMismatch)
    );
    assert!(matches!(late.run_once(), Step::Advanced { .. }));
    assert_eq!(
        late.bind_verification_scope(
            &grant(context.owner.authority),
            binding(context.authenticator.id)
        ),
        Err(ObservationError::ContextMismatch)
    );
}
