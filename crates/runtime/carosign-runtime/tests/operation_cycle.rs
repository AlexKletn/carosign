//! Operation cycle: order of steps and place of failure.
//!
//! ⚠️ What is being tested is what will be the first live path in the service: admission →
//! set → actor’s step → summation, and that each refusal claims its place.

#[path = "operation_cycle_fixtures/allow.rs"]
mod allow;
#[path = "operation_cycle_fixtures/deny.rs"]
mod deny;
#[path = "operation_cycle_fixtures/identity.rs"]
mod identity;
#[path = "operation_cycle_fixtures/wrong_scope.rs"]
mod wrong_scope;

use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
use carosign_core::identity::{SubjectRef, SubjectSelector};
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, BootId, EnrollmentId, GrantId, OperationId, SubjectId,
};
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, IdentityAuthority, PolicyAuthority, TransactionId,
};
use carosign_runtime::{
    BindingKind, CycleError, CycleStep, EnrollmentBinding, OperationActor, OperationMsg,
    PermittedSet, VerificationCycle, step_actor,
};

use deny::Deny;

use allow::Allow;

use wrong_scope::WrongScope;

fn verify_scope(subject: SubjectRef) -> GrantScope {
    GrantScope::Verify {
        subject,
        authenticator: AuthenticatorId::from_bytes([6; 16]).unwrap(),
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
    }
}

use identity::Identity;

fn caller() -> CallerContext {
    CallerContext::from_transport(
        [0u8; 32],
        1000,
        TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
    )
}

fn set() -> PermittedSet {
    PermittedSet::new(vec![EnrollmentBinding {
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([5u8; 16]).expect("валид"),
            authenticator: AuthenticatorId::from_bytes([6u8; 16]).expect("валид"),
            generation: EnrollmentGeneration::FIRST,
        },
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![1, 2, 3],
        format_id: None,
        format_version: 0,
    }])
    .expect("набор")
}

#[test]
fn refused_authority_stops_the_cycle() {
    let cycle = VerificationCycle::new(
        caller(),
        verify_scope(SubjectRef {
            authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
        }),
        set(),
    );
    let result = futures_executor::block_on(cycle.authorize(&Deny));
    assert_eq!(result, Err(CycleError::Refused(AuthorityRefusal::Denied)));
}

#[test]
fn allowed_authority_lets_the_cycle_go_on() {
    let cycle = VerificationCycle::new(
        caller(),
        verify_scope(SubjectRef {
            authority: AuthorityId::from_bytes([2u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([4u8; 16]).expect("валид"),
        }),
        set(),
    );
    assert!(futures_executor::block_on(cycle.authorize(&Allow)).is_ok());
    // ⚠️ The plan carries a set - there is no empty.
    match cycle.plan() {
        carosign_runtime::ports::CommandPlan::Verify { permitted } => {
            assert_eq!(permitted.len(), 1)
        }
        other => panic!("ожидалась проверка, а пришло {other:?}"),
    }
}

#[test]
fn authority_cannot_return_grant_for_a_different_subject() {
    let cycle = VerificationCycle::new(
        caller(),
        verify_scope(SubjectRef {
            authority: AuthorityId::from_bytes([2; 16]).unwrap(),
            subject: SubjectId::from_bytes([4; 16]).unwrap(),
        }),
        set(),
    );
    assert_eq!(
        futures_executor::block_on(cycle.authorize(&WrongScope)),
        Err(CycleError::Refused(AuthorityRefusal::Denied))
    );
}

#[test]
fn verification_rejects_other_action_or_device_even_if_authority_would_allow() {
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([2; 16]).unwrap(),
        subject: SubjectId::from_bytes([4; 16]).unwrap(),
    };
    let wrong_device = GrantScope::Verify {
        subject,
        authenticator: AuthenticatorId::from_bytes([8; 16]).unwrap(),
        purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
    };
    let other_action = GrantScope::Observe(OperationId::from_bytes([9; 16]).unwrap());
    for scope in [wrong_device, other_action] {
        let cycle = VerificationCycle::new(caller(), scope, set());
        assert_eq!(
            futures_executor::block_on(cycle.authorize(&Allow)),
            Err(CycleError::Refused(AuthorityRefusal::Denied))
        );
    }
}

#[test]
fn actor_step_moves_the_operation() {
    use carosign_engine::tests_support::initial;

    let mut actor = OperationActor::new(initial());
    let step = step_actor(
        &mut actor,
        OperationMsg::Domain(carosign_engine::DomainInput::Authorized),
    );
    match step {
        CycleStep::Progressed { phase, effects } => {
            assert_eq!(phase, "WaitingLease");
            assert_eq!(effects, 1);
        }
        other => panic!("ожидался переход, а пришло {other:?}"),
    }
    // The move of the provider before sending is unacceptable.
    assert!(matches!(
        step_actor(
            &mut actor,
            OperationMsg::Domain(carosign_engine::DomainInput::Progress)
        ),
        CycleStep::Refused(_)
    ));
}

#[test]
fn operation_id_comes_from_the_actor() {
    use carosign_engine::tests_support::initial;

    let actor = OperationActor::new(initial());
    let id: OperationId = carosign_runtime::flow::cycle::operation_of(&actor);
    assert_eq!(id, initial().common.context.operation_id);
}

#[test]
fn identity_port_resolves_inside_the_cycle() {
    let subject =
        futures_executor::block_on(Identity.resolve(&caller(), &SubjectSelector::Current))
            .expect("субъект");
    assert_eq!(subject.subject.as_bytes()[0], 4);
}
