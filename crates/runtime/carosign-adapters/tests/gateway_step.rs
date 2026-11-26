//! Gateway: Actor's Step → Effects → Watching Back Into The Actor
//!
//! ⚠️ The main property is checked: the state is changed by the actor, not the performer, and
//! the changing sending is not repeated.

use carosign_adapters::gateway::{Gateway, GatewayStep};
use carosign_core::ids::OperationId;
use carosign_engine::tests_support::{context, initial};
use carosign_engine::{
    DomainInput, EffectReceipt, EffectSpec, OperationPhase, PlannedEffect, TransitionIndex,
};
use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner, Execution};
use carosign_runtime::ports::CommandPlan;
use carosign_runtime::{OperationActor, OperationMsg};

#[path = "fixtures/gateway_step/provider.rs"]
mod provider;
use provider::Provider;
#[path = "fixtures/gateway_step/journal.rs"]
mod journal;
use journal::Journal;
#[path = "fixtures/gateway_step/failing_journal.rs"]
mod failing_journal;
use failing_journal::FailingJournal;
#[path = "fixtures/gateway_step/cancel_failed.rs"]
mod cancel_failed;
use cancel_failed::CancelFailed;

#[test]
fn failed_cancel_does_not_claim_provider_lacks_support() {
    let provider = CancelFailed;
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let effect = PlannedEffect::at(EffectSpec::RequestCancel, TransitionIndex::new(1), 0);
    let result = futures_executor::block_on(
        runner.execute(effect, OperationId::from_bytes([7; 16]).unwrap()),
    );
    assert_eq!(result, Execution::Noop);
    assert_eq!(runner.receipt_for(EffectSpec::RequestCancel, &result), None);
}

#[test]
fn gateway_and_runner_refuse_another_operation_before_work() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let first = OperationId::from_bytes([1; 16]).unwrap();
    let second = OperationId::from_bytes([2; 16]).unwrap();
    let noop = PlannedEffect::at(EffectSpec::AcquireResources, TransitionIndex::new(1), 0);
    assert_eq!(
        futures_executor::block_on(runner.execute(noop, first)),
        Execution::Noop
    );
    let persist = PlannedEffect::at(EffectSpec::PersistIntent, TransitionIndex::new(1), 0);
    assert_eq!(
        futures_executor::block_on(runner.execute(persist, second)),
        Execution::NoDispatchPlan
    );
    assert_eq!(journal.0.load(std::sync::atomic::Ordering::SeqCst), 0);

    let mut gateway = Gateway::new(EffectRunner::new(
        &provider,
        &journal,
        DispatchPlans::default(),
    ));
    let mut a = OperationActor::new(initial());
    let mut other_state = initial();
    other_state.common.context.operation_id = second;
    let mut b = OperationActor::new(other_state);
    assert_eq!(
        futures_executor::block_on(gateway.step(&mut a)),
        GatewayStep::Idle
    );
    b.post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    let before = b.state().clone();
    assert!(matches!(
        futures_executor::block_on(gateway.step(&mut b)),
        GatewayStep::Refused(_)
    ));
    assert_eq!(b.state(), &before);
    assert!(b.has_work());
    let mut hub = carosign_runtime::operation::hub::EventHub::new(second);
    assert!(matches!(
        futures_executor::block_on(gateway.step_observed(&mut b, &mut hub)),
        Err(carosign_runtime::operation::hub::AttachError::WrongOperation)
    ));
}

#[test]
fn failed_durable_effect_never_produces_a_success_receipt() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = FailingJournal;
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let operation = OperationId::from_bytes([7; 16]).expect("валид");
    for spec in [
        EffectSpec::PersistIntent,
        EffectSpec::PersistNeedsReconciliation,
        EffectSpec::RecordTerminal,
    ] {
        let effect = PlannedEffect::at(spec, TransitionIndex::INITIAL, 0);
        let execution = futures_executor::block_on(runner.execute(effect, operation));
        assert_eq!(execution, Execution::StorageUnavailable, "{spec:?}");
        assert_eq!(runner.receipt_for(spec, &execution), None, "{spec:?}");
        assert_eq!(
            runner.receipt_for(spec, &Execution::NoDispatchPlan),
            None,
            "{spec:?}"
        );
    }
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn gateway_does_not_report_mutation_terminal_before_record_is_confirmed() {
    use carosign_core::enrollment::EnrollmentLabel;
    use carosign_core::ids::MutationId;
    use carosign_core::operation::mutation::EnrollRequest;
    use carosign_engine::{OperationState, TerminalOutcome};

    let context = initial().common.context;
    let request = EnrollRequest {
        subject: context.subject,
        authenticator: context.authenticator.id,
        target: EnrollmentLabel::new("test").unwrap(),
        replace: None,
    };
    let mut state =
        OperationState::enrollment(context, request, MutationId::from_bytes([7; 16]).unwrap());
    state.common.phase = OperationPhase::Terminal;
    state.common.terminal = Some(TerminalOutcome::Cancelled);
    state.common.transition_index = state.common.transition_index.next().unwrap();
    state.common.pending_effects.push(PlannedEffect::at(
        EffectSpec::RecordTerminal,
        state.common.transition_index,
        0,
    ));
    let mut actor = OperationActor::new(state);
    actor
        .post(OperationMsg::Domain(DomainInput::StorageUnavailable))
        .unwrap();
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = FailingJournal;
    let mut gateway = Gateway::new(EffectRunner::new(
        &provider,
        &journal,
        DispatchPlans::default(),
    ));
    assert!(matches!(
        futures_executor::block_on(gateway.step(&mut actor)),
        GatewayStep::Refused(_)
    ));
    let mut restored = actor.state().clone();
    restored.common.pending_effects.clear();
    let mut actor = OperationActor::new(restored);
    actor
        .post(OperationMsg::Domain(DomainInput::Progress))
        .unwrap();
    assert!(matches!(
        futures_executor::block_on(gateway.step(&mut actor)),
        GatewayStep::Refused(_)
    ));
}

#[test]
fn gateway_reports_storage_loss_to_mutation_actor() {
    use carosign_core::enrollment::EnrollmentLabel;
    use carosign_core::ids::MutationId;
    use carosign_core::operation::mutation::EnrollRequest;
    use carosign_engine::OperationState;
    use carosign_runtime::operation::actor::Step;

    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = FailingJournal;
    let runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let mut gateway = Gateway::new(runner);
    let context = initial().common.context;
    let request = EnrollRequest {
        subject: context.subject,
        authenticator: context.authenticator.id,
        target: EnrollmentLabel::new("").unwrap(),
        replace: None,
    };
    let mut actor = OperationActor::new(OperationState::enrollment(
        context,
        request,
        MutationId::from_bytes([7; 16]).unwrap(),
    ));
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    assert_eq!(
        futures_executor::block_on(gateway.step(&mut actor)),
        GatewayStep::StorageUnavailable
    );
    let old_intent = *actor
        .state()
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::PersistIntent)
        .unwrap();
    actor
        .post(OperationMsg::Domain(DomainInput::Receipt {
            effect: old_intent,
            receipt: EffectReceipt::IntentDurable,
        }))
        .unwrap();
    let Step::Advanced { state, effects } = actor.run_once() else {
        panic!("ожидалась сверка")
    };
    assert_eq!(state.common.phase, OperationPhase::Reconciling);
    assert!(
        !state
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::PersistIntent)
    );
    assert!(
        effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
    assert!(
        !effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::PersistNeedsReconciliation)
    );
    assert!(matches!(actor.run_once(), Step::Rejected(_)));
    assert_eq!(actor.state().common.phase, OperationPhase::Reconciling);
}

#[test]
fn unconfirmed_cancel_does_not_skip_persisting_mutation_uncertainty() {
    use carosign_core::{
        enrollment::EnrollmentLabel, ids::MutationId, operation::mutation::EnrollRequest,
    };
    use carosign_engine::OperationState;

    let context = initial().common.context;
    let request = EnrollRequest {
        subject: context.subject,
        authenticator: context.authenticator.id,
        target: EnrollmentLabel::new("test").unwrap(),
        replace: None,
    };
    let mut state =
        OperationState::enrollment(context, request, MutationId::from_bytes([7; 16]).unwrap());
    state.common.phase = OperationPhase::InFlight;
    let mut actor = OperationActor::new(state);
    actor.post(OperationMsg::CancelRequested).unwrap();
    let mut gateway = Gateway::new(EffectRunner::new(
        &CancelFailed,
        &FailingJournal,
        DispatchPlans::default(),
    ));
    assert_eq!(
        futures_executor::block_on(gateway.step(&mut actor)),
        GatewayStep::StorageUnavailable
    );
    assert!(actor.has_work());
}

#[test]
fn durable_success_still_produces_corresponding_receipt() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let operation = OperationId::from_bytes([7; 16]).expect("валид");
    for (spec, receipt) in [
        (EffectSpec::PersistIntent, EffectReceipt::IntentDurable),
        (
            EffectSpec::PersistNeedsReconciliation,
            EffectReceipt::ReconciliationPersisted,
        ),
        (EffectSpec::RecordTerminal, EffectReceipt::MetadataDurable),
    ] {
        let execution = futures_executor::block_on(runner.execute(
            PlannedEffect::at(spec, TransitionIndex::INITIAL, 0),
            operation,
        ));
        assert_eq!(
            runner.receipt_for(spec, &execution),
            Some(receipt),
            "{spec:?}"
        );
    }
}

#[test]
fn unconnected_arm_port_never_authorizes_a_mutating_send() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let spec = EffectSpec::PersistDispatchArmed;
    let effect = PlannedEffect::at(spec, TransitionIndex::INITIAL, 0);
    let execution = futures_executor::block_on(runner.execute(effect, operation_id_for_test()));
    assert_eq!(execution, Execution::NoDispatchPlan);
    assert_eq!(runner.receipt_for(spec, &execution), None);
    assert_eq!(
        runner.receipt_for(
            spec,
            &Execution::Receipt(EffectReceipt::DispatchArmedDurable)
        ),
        None
    );
    assert_eq!(journal.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn noop_never_proves_a_durable_effect_or_cancellation() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    for spec in [
        EffectSpec::PersistIntent,
        EffectSpec::PersistDispatchArmed,
        EffectSpec::CommitDesiredRegistryAndJournal,
        EffectSpec::CommitNoChangeAndJournal,
        EffectSpec::PersistNeedsReconciliation,
        EffectSpec::RecordTerminal,
        EffectSpec::RequestCancel,
    ] {
        assert_eq!(runner.receipt_for(spec, &Execution::Noop), None, "{spec:?}");
    }
}

#[test]
fn typed_mutation_commits_are_not_acknowledged_without_a_verified_port() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let operation = operation_id_for_test();
    for spec in [
        EffectSpec::CommitDesiredRegistryAndJournal,
        EffectSpec::CommitNoChangeAndJournal,
    ] {
        let execution = futures_executor::block_on(runner.execute(
            PlannedEffect::at(spec, TransitionIndex::new(1), 0),
            operation,
        ));
        assert_eq!(execution, Execution::Noop);
        assert_eq!(runner.receipt_for(spec, &execution), None);
        assert_eq!(
            runner.receipt_for(spec, &Execution::Receipt(EffectReceipt::MetadataDurable)),
            None
        );
    }
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn query_without_verified_safe_capability_does_not_call_provider() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let effect = PlannedEffect::at(EffectSpec::QueryProviderState, TransitionIndex::new(1), 0);
    let result = futures_executor::block_on(runner.execute(effect, operation_id_for_test()));
    assert_eq!(result, Execution::Receipt(EffectReceipt::StateUnknown));
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

fn operation_id_for_test() -> OperationId {
    OperationId::from_bytes([7; 16]).unwrap()
}

#[test]
fn receipt_must_match_effect_type() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let sent = Execution::Receipt(EffectReceipt::SendAccepted);
    assert_eq!(
        runner.receipt_for(EffectSpec::SendVerify, &sent),
        Some(EffectReceipt::SendAccepted)
    );
    assert_eq!(
        runner.receipt_for(EffectSpec::SendMutatingCommand, &sent),
        None
    );
    assert_eq!(
        runner.receipt_for(EffectSpec::PersistDispatchArmed, &sent),
        None
    );
    let durable = Execution::Receipt(EffectReceipt::MetadataDurable);
    assert_eq!(
        runner.receipt_for(EffectSpec::RecordTerminal, &durable),
        Some(EffectReceipt::MetadataDurable)
    );
    assert_eq!(
        runner.receipt_for(EffectSpec::PersistIntent, &durable),
        None
    );
}

#[test]
fn step_moves_actor_and_returns_receipts() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let mut gateway = Gateway::new(runner);

    let mut actor = OperationActor::new(initial());
    // ⚠️ Before admission, the step does nothing.
    assert_eq!(
        futures_executor::block_on(gateway.step(&mut actor)),
        GatewayStep::Idle
    );

    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .expect("почта принимает");
    let step = futures_executor::block_on(gateway.step(&mut actor));
    assert_eq!(step, GatewayStep::Refused("эффект не исполнен"));
    assert_eq!(actor.state().common.phase, OperationPhase::WaitingLease);
    assert!(
        actor
            .state()
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::AcquireResources)
    );
    let _ = context();
}

#[test]
fn observed_gateway_reports_slow_client_without_losing_the_actor_effect() {
    use carosign_runtime::operation::{
        events::EventKind, hub::EventHub, observer::OBSERVER_QUEUE_MAX,
    };
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let mut gateway = Gateway::new(EffectRunner::new(
        &provider,
        &journal,
        DispatchPlans::default(),
    ));
    let mut actor = OperationActor::new(initial());
    let mut hub = EventHub::new(actor.state().common.context.operation_id);
    hub.attach(1, 0).unwrap();
    for _ in 0..OBSERVER_QUEUE_MAX {
        hub.publish(OperationPhase::New, EventKind::StateChanged)
            .unwrap();
    }
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    let result = futures_executor::block_on(gateway.step_observed(&mut actor, &mut hub)).unwrap();
    assert_eq!(result.notification.unwrap(), vec![1]);
    assert_eq!(result.step, GatewayStep::Refused("эффект не исполнен"));
    assert!(
        actor
            .pending()
            .iter()
            .any(|effect| effect.spec == EffectSpec::AcquireResources)
    );
    assert_eq!(actor.state().common.phase, OperationPhase::WaitingLease);
}

#[test]
fn mutating_send_without_armed_ticket_is_refused() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let plan = CommandPlan::Mutate {
        mutation: carosign_core::ids::MutationId::from_bytes([1; 16]).unwrap(),
        command: carosign_core::ids::CommandId::from_bytes([2; 16]).unwrap(),
        authorization: carosign_runtime::ports::EnrollmentAuthorization::None,
        change: carosign_runtime::ports::MutationChange::Create {
            planned_new: carosign_core::enrollment::EnrollmentRef {
                id: carosign_core::ids::EnrollmentId::from_bytes([3; 16]).unwrap(),
                authenticator: carosign_core::ids::AuthenticatorId::from_bytes([4; 16]).unwrap(),
                generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
            },
        },
    };
    let mut runner = EffectRunner::new(
        &provider,
        &journal,
        DispatchPlans {
            mutate: Some(plan),
            ..DispatchPlans::default()
        },
    );
    let operation = OperationId::from_bytes([7; 16]).unwrap();
    let effect = PlannedEffect::at(EffectSpec::SendMutatingCommand, TransitionIndex::new(1), 0);
    let first = futures_executor::block_on(runner.execute(effect, operation));
    let second = futures_executor::block_on(runner.execute(effect, operation));
    assert_eq!(first, Execution::NoDispatchPlan);
    assert_eq!(second, Execution::NoDispatchPlan);
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn verify_effect_rejects_non_verification_commands_before_provider() {
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let operation = OperationId::from_bytes([7; 16]).unwrap();
    let mutation = carosign_core::ids::MutationId::from_bytes([1; 16]).unwrap();
    let plans = [
        CommandPlan::Query,
        CommandPlan::PrepareEnrollmentChallenge { mutation },
        CommandPlan::Mutate {
            mutation,
            command: carosign_core::ids::CommandId::from_bytes([2; 16]).unwrap(),
            authorization: carosign_runtime::ports::EnrollmentAuthorization::None,
            change: carosign_runtime::ports::MutationChange::Create {
                planned_new: carosign_core::enrollment::EnrollmentRef {
                    id: carosign_core::ids::EnrollmentId::from_bytes([3; 16]).unwrap(),
                    authenticator: carosign_core::ids::AuthenticatorId::from_bytes([4; 16])
                        .unwrap(),
                    generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
                },
            },
        },
    ];
    for plan in plans {
        let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::verify(plan));
        let effect = PlannedEffect::at(EffectSpec::SendVerify, TransitionIndex::new(1), 0);
        assert_eq!(
            futures_executor::block_on(runner.execute(effect, operation)),
            Execution::NoDispatchPlan
        );
    }
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn verify_plan_is_consumed_before_dispatch_and_cannot_be_replayed() {
    use carosign_runtime::{BindingKind, EnrollmentBinding, PermittedSet};
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal(std::sync::atomic::AtomicUsize::new(0));
    let permitted = PermittedSet::new(vec![EnrollmentBinding {
        enrollment: carosign_core::enrollment::EnrollmentRef {
            id: carosign_core::ids::EnrollmentId::from_bytes([3; 16]).unwrap(),
            authenticator: carosign_core::ids::AuthenticatorId::from_bytes([4; 16]).unwrap(),
            generation: carosign_core::enrollment::EnrollmentGeneration::FIRST,
        },
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![1],
        format_id: None,
        format_version: 0,
    }])
    .unwrap();
    let mut cancelled = EffectRunner::new(
        &provider,
        &journal,
        DispatchPlans::verify(CommandPlan::Verify {
            permitted: permitted.clone(),
        }),
    );
    let operation = OperationId::from_bytes([7; 16]).unwrap();
    let cancel = PlannedEffect::at(EffectSpec::RequestCancel, TransitionIndex::new(1), 0);
    assert_eq!(
        futures_executor::block_on(cancelled.execute(cancel, operation)),
        Execution::Receipt(EffectReceipt::CancelDispatched)
    );
    let effect = PlannedEffect::at(EffectSpec::SendVerify, TransitionIndex::new(2), 0);
    assert_eq!(
        futures_executor::block_on(cancelled.execute(effect, operation)),
        Execution::NoDispatchPlan
    );
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let mut runner = EffectRunner::new(
        &provider,
        &journal,
        DispatchPlans::verify(CommandPlan::Verify { permitted }),
    );
    let effect = PlannedEffect::at(EffectSpec::SendVerify, TransitionIndex::new(1), 0);
    assert_eq!(
        futures_executor::block_on(runner.execute(effect, operation)),
        Execution::Receipt(EffectReceipt::SendAccepted)
    );
    assert_eq!(
        futures_executor::block_on(runner.execute(effect, operation)),
        Execution::NoDispatchPlan
    );
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 1);
}
