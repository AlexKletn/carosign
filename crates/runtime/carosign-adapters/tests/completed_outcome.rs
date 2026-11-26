use carosign_adapters::mutation::{OutcomeReadError, authorized_completed_outcome};
use carosign_core::authorization::GrantScope;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, EnrollmentId, MutationId, OperationId, SubjectId,
};
use carosign_persistence::model::{MutationPhase, MutationRecord, MutationTerminalOutcome};
use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
use carosign_persistence::store::{MetadataStore, Table, tests_support::MemoryStore};
use carosign_runtime::ports::{AuthorityRefusal, CallerContext, TransactionId};

// An old source can confirm the grant without confirming its caller.

#[path = "fixtures/completed_outcome/policy.rs"]
mod policy;
use policy::Policy;
#[path = "fixtures/completed_outcome/legacy_policy.rs"]
mod legacy_policy;
use legacy_policy::LegacyPolicy;

#[test]
fn durable_outcome_requires_authorization_for_the_exact_operation() {
    let store = MemoryStore::default();
    let operation = OperationId::from_bytes([4; 16]).unwrap();
    let mutation = MutationId::from_bytes([5; 16]).unwrap();
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([6; 16]).unwrap(),
        subject: SubjectId::from_bytes([7; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([8; 16]).unwrap();
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([9; 16]).unwrap()),
        1,
        vec![],
        Scope {
            subject,
            authenticator,
        },
    )
    .unwrap();
    let record = MutationRecord {
        mutation,
        operation,
        subject,
        authenticator,
        expected_generation: None,
        phase: MutationPhase::Completed,
        worker: None,
        epoch: None,
        command: None,
        recovery_ref: None,
        provider_observation_digest: None,
        terminal_outcome: Some(MutationTerminalOutcome::NoChange),
        plan: Some(plan),
    };
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        mutation.as_bytes(),
        &carosign_persistence::mutation_records::encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    let caller =
        CallerContext::from_transport([0; 32], 1000, TransactionId::from_bytes([10; 16]).unwrap());
    let legacy = LegacyPolicy(Policy {
        scope: GrantScope::Observe(operation),
        revoked: false,
    });
    assert_eq!(
        futures_executor::block_on(authorized_completed_outcome(
            &legacy, &caller, &store, mutation, operation,
        )),
        Err(OutcomeReadError::Authority(AuthorityRefusal::Unavailable))
    );
    assert_eq!(
        futures_executor::block_on(
            carosign_adapters::mutation::authorized_completed_outcome_by_operation(
                &legacy, &caller, &store, operation,
            )
        ),
        Err(OutcomeReadError::Authority(AuthorityRefusal::Unavailable))
    );
    let run = |scope, revoked, requested_mutation| {
        futures_executor::block_on(authorized_completed_outcome(
            &Policy { scope, revoked },
            &caller,
            &store,
            requested_mutation,
            operation,
        ))
    };
    assert_eq!(
        run(GrantScope::Observe(operation), false, mutation),
        Ok(MutationTerminalOutcome::NoChange)
    );
    assert_eq!(
        run(
            GrantScope::Observe(OperationId::from_bytes([11; 16]).unwrap()),
            false,
            mutation
        ),
        Err(OutcomeReadError::Authority(AuthorityRefusal::Denied))
    );
    assert_eq!(
        run(GrantScope::Observe(operation), true, mutation),
        Err(OutcomeReadError::Authority(AuthorityRefusal::Revoked))
    );
    assert!(matches!(
        run(
            GrantScope::Observe(operation),
            false,
            MutationId::from_bytes([12; 16]).unwrap()
        ),
        Err(OutcomeReadError::Storage(_))
    ));
    use carosign_adapters::mutation::CompletedMutationReader;
    use carosign_runtime::operation::{
        events::EventKind,
        hub::EventHub,
        observer::{AuthorizedSubscription, SubscriptionError},
    };
    let mut hub = EventHub::new(operation);
    let policy = Policy {
        scope: GrantScope::Observe(operation),
        revoked: false,
    };
    let subscription = futures_executor::block_on(AuthorizedSubscription::attach(
        &policy, &caller, &mut hub, 1, 0,
    ))
    .unwrap();
    hub.publish(
        carosign_engine::OperationPhase::Terminal,
        EventKind::Terminal,
    )
    .unwrap();
    let reader = CompletedMutationReader::new(&store);
    let wrong_mutation = MutationId::from_bytes([12; 16]).unwrap();
    assert_eq!(
        futures_executor::block_on(subscription.next_mutation_terminal(
            &policy,
            &reader,
            wrong_mutation,
            &mut hub
        )),
        Err(SubscriptionError::TerminalUnavailable)
    );
    let (event, outcome) = futures_executor::block_on(
        subscription.next_mutation_terminal(&policy, &reader, mutation, &mut hub),
    )
    .unwrap()
    .unwrap();
    assert_eq!(event.sequence, 1);
    assert_eq!(outcome, carosign_engine::TerminalOutcome::NoChange);
    assert!(
        futures_executor::block_on(
            subscription.next_mutation_terminal(&policy, &reader, mutation, &mut hub)
        )
        .unwrap()
        .is_none()
    );
    let mut revoked_hub = EventHub::new(operation);
    let revoked_subscription = futures_executor::block_on(AuthorizedSubscription::attach(
        &policy,
        &caller,
        &mut revoked_hub,
        1,
        0,
    ))
    .unwrap();
    revoked_hub
        .publish(
            carosign_engine::OperationPhase::Terminal,
            EventKind::Terminal,
        )
        .unwrap();
    let revoked_policy = Policy {
        scope: GrantScope::Observe(operation),
        revoked: true,
    };
    assert_eq!(
        futures_executor::block_on(revoked_subscription.next_mutation_terminal(
            &revoked_policy,
            &reader,
            wrong_mutation,
            &mut revoked_hub
        )),
        Err(SubscriptionError::Authority(AuthorityRefusal::Revoked))
    );
    assert_eq!(
        futures_executor::block_on(revoked_subscription.next_mutation_terminal(
            &policy,
            &reader,
            mutation,
            &mut revoked_hub
        )),
        Err(SubscriptionError::Detached)
    );
    use carosign_core::{enrollment::EnrollmentLabel, operation::mutation::EnrollRequest};
    use carosign_runtime::operation::events::{EventStream, HISTORY_MAX, Replay};
    let mut context = carosign_engine::tests_support::initial().common.context;
    context.operation_id = operation;
    context.subject = subject;
    context.authenticator.id = authenticator;
    let mut state = carosign_engine::OperationState::enrollment(
        context,
        EnrollRequest {
            subject,
            authenticator,
            target: EnrollmentLabel::new("палец").unwrap(),
            replace: None,
        },
        mutation,
    );
    state.common.phase = carosign_engine::OperationPhase::Terminal;
    state.common.terminal = Some(carosign_engine::TerminalOutcome::MutationCommitted);
    let mut confirmed_hub = EventHub::new(operation);
    let confirmed_subscription = futures_executor::block_on(AuthorizedSubscription::attach(
        &policy,
        &caller,
        &mut confirmed_hub,
        3,
        0,
    ))
    .unwrap();
    let mut incorrect_state = state.clone();
    incorrect_state.body.mutation_mut().unwrap().mutation = wrong_mutation;
    assert!(
        futures_executor::block_on(
            confirmed_hub.publish_confirmed_mutation_terminal(&incorrect_state, &reader)
        )
        .is_err()
    );
    assert_eq!(confirmed_hub.stream().last_sequence(), 0);
    assert!(
        futures_executor::block_on(
            confirmed_hub.publish_confirmed_mutation_terminal(&state, &reader)
        )
        .unwrap()
        .1
        .is_empty()
    );
    assert_eq!(
        futures_executor::block_on(
            confirmed_hub.publish_confirmed_mutation_terminal(&state, &reader)
        )
        .unwrap(),
        (carosign_engine::TerminalOutcome::NoChange, vec![])
    );
    assert_eq!(confirmed_hub.stream().last_sequence(), 1);
    assert_eq!(
        futures_executor::block_on(confirmed_subscription.next_mutation_terminal(
            &policy,
            &reader,
            mutation,
            &mut confirmed_hub
        ))
        .unwrap()
        .unwrap()
        .1,
        carosign_engine::TerminalOutcome::NoChange
    );
    let mut stream = EventStream::new(operation);
    for _ in 0..=HISTORY_MAX {
        stream
            .publish(
                carosign_engine::OperationPhase::Running,
                EventKind::StateChanged,
            )
            .unwrap();
    }
    let replay =
        futures_executor::block_on(carosign_adapters::mutation::authorized_completed_replay(
            &Policy {
                scope: GrantScope::Observe(operation),
                revoked: false,
            },
            &caller,
            &store,
            &stream,
            &state,
            0,
        ))
        .unwrap();
    assert!(
        matches!(replay.replay, Replay::Gap { snapshot, .. } if snapshot.terminal == Some(carosign_engine::TerminalOutcome::NoChange))
    );
    assert!(
        futures_executor::block_on(carosign_adapters::mutation::authorized_completed_replay(
            &Policy {
                scope: GrantScope::Observe(operation),
                revoked: true
            },
            &caller,
            &store,
            &stream,
            &state,
            0,
        ))
        .is_err()
    );
    use carosign_adapters::mutation::authorized_completed_outcome_by_operation;
    let lookup = |policy: &Policy, id| {
        futures_executor::block_on(authorized_completed_outcome_by_operation(
            policy, &caller, &store, id,
        ))
    };
    assert_eq!(
        lookup(&policy, operation),
        Ok(Some(MutationTerminalOutcome::NoChange))
    );
    assert_eq!(
        lookup(&revoked_policy, operation),
        Err(OutcomeReadError::Authority(AuthorityRefusal::Revoked))
    );
    let absent = OperationId::from_bytes([44; 16]).unwrap();
    assert_eq!(
        lookup(
            &Policy {
                scope: GrantScope::Observe(absent),
                revoked: false
            },
            absent
        ),
        Ok(None)
    );
    let mut duplicate = record.clone();
    duplicate.mutation = MutationId::from_bytes([45; 16]).unwrap();
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        duplicate.mutation.as_bytes(),
        &carosign_persistence::mutation_records::encode_mutation(&duplicate).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert!(matches!(
        lookup(&policy, operation),
        Err(OutcomeReadError::Storage(_))
    ));
}
