//! Service in its entirety: request → steps → outcome, in real storage.
//!
//! ⚠️ The path that used to stop at “operation accepted” is being tested: now steps are being
//! taken and the outcome is visible.

#[path = "service_run_fixtures/advancing_clock.rs"]
mod advancing_clock;
#[path = "service_run_fixtures/allow.rs"]
mod allow;
#[path = "service_run_fixtures/clock.rs"]
mod clock;
#[path = "service_run_fixtures/delayed_grant.rs"]
mod delayed_grant;
#[path = "service_run_fixtures/failed_transport.rs"]
mod failed_transport;
#[path = "service_run_fixtures/failing_ids.rs"]
mod failing_ids;
#[path = "service_run_fixtures/foreign_owner.rs"]
mod foreign_owner;
#[path = "service_run_fixtures/ids.rs"]
mod ids;
#[path = "service_run_fixtures/journal.rs"]
mod journal;
#[path = "service_run_fixtures/missing_profile.rs"]
mod missing_profile;
#[path = "service_run_fixtures/never_broker.rs"]
mod never_broker;
#[path = "service_run_fixtures/provider.rs"]
mod provider;
#[path = "service_run_fixtures/revoke_after_read.rs"]
mod revoke_after_read;
#[path = "service_run_fixtures/revoked.rs"]
mod revoked;
#[path = "service_run_fixtures/tracking_authority.rs"]
mod tracking_authority;
#[path = "service_run_fixtures/waiting_broker.rs"]
mod waiting_broker;

use carosign_core::authorization::{AuthorizationBoundary, GrantScope, ValidatedGrant};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef, OpaqueProviderRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::CommandId;
use carosign_core::ids::IdSource;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, BootId, ConnectionId, EnrollmentId, EvidenceId, GrantId,
    IdempotencyKey, LeaseId, MutationId, OperationId, SubjectId, WorkerInstanceId,
};
use carosign_core::operation::context::BootInstant;
use carosign_engine::OperationPhase;
use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
use carosign_persistence::mutation_plan::StoredBindingKind;
use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::store::{MetadataStore, Table};
use carosign_runtime::ports::{
    AuthorityRefusal, BoxFuture, CallerContext, CommandPlan, DispatchOutcome, DurableMarker,
    MutationPort, PolicyAuthority, ProviderExecutionPort, TimeSource, TransactionId,
};
use carosignd::run::RunOutcome;
use carosignd::service::{Served, Service};
use std::path::PathBuf;

mod support;
use support::GrantAll;

fn matching_grant(state: &carosign_engine::OperationState) -> ValidatedGrant {
    ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        AuthorityId::from_bytes([2; 16]).unwrap(),
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        1000,
    )
}

#[test]
fn verification_terminal_cannot_be_confirmed_by_generic_mutation_port() {
    use carosign_engine::{DomainInput, EffectReceipt, EffectSpec};
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    use carosign_runtime::{OperationActor, OperationMsg};

    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let mut runner = EffectRunner::new(&provider, &Journal, DispatchPlans::default());
    let mut actor = OperationActor::new(carosign_engine::tests_support::initial());
    let grant = matching_grant(actor.state());
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    let outcome = futures_executor::block_on(carosignd::run::run(
        &mut actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, spec| (spec == EffectSpec::AcquireResources).then_some(EffectReceipt::LeaseDenied),
    ));
    assert!(matches!(
        outcome,
        RunOutcome::Waiting {
            phase: OperationPhase::Finalizing
        }
    ));
    assert!(actor.expects(EffectSpec::RecordTerminal));
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn progress_does_not_reexecute_an_unconfirmed_send() {
    use carosign_engine::{
        DomainInput, EffectReceipt, EffectSpec, PlannedEffect, TransitionIndex, decide,
    };
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    use carosign_runtime::{OperationActor, OperationMsg};

    let mut state = carosign_engine::tests_support::initial();
    state = decide(&state, DomainInput::Authorized).unwrap().next;
    for (index, spec, receipt) in [
        (1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        (2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
    ] {
        state = decide(
            &state,
            DomainInput::Receipt {
                effect: PlannedEffect::at(spec, TransitionIndex::new(index), 0),
                receipt,
            },
        )
        .unwrap()
        .next;
    }
    let mut actor = OperationActor::new(state);
    actor
        .post(OperationMsg::Domain(DomainInput::Progress))
        .unwrap();
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let grant = matching_grant(actor.state());
    let outcome = futures_executor::block_on(carosignd::run::run(
        &mut actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, _| None,
    ));
    // The posting is not connected yet; the opt-out does not have to resend the command.
    assert!(
        matches!(outcome, RunOutcome::Refused { reason, .. } if reason == "эффект не исполнен")
    );
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(actor.expects(EffectSpec::SendVerify));
}

#[test]
fn resource_observer_cannot_supply_a_foreign_receipt() {
    use carosign_engine::{DomainInput, EffectReceipt};
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    use carosign_runtime::{OperationActor, OperationMsg};

    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let mut actor = OperationActor::new(carosign_engine::tests_support::initial());
    let grant = matching_grant(actor.state());
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    let result = futures_executor::block_on(carosignd::run::run(
        &mut actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, _| Some(EffectReceipt::SendAccepted),
    ));
    assert!(matches!(result, RunOutcome::Refused { .. }));
    assert_eq!(actor.state().common.phase, OperationPhase::WaitingLease);
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn terminal_mutation_without_pending_effect_still_requires_durable_result() {
    use carosign_core::enrollment::EnrollmentLabel;
    use carosign_core::operation::mutation::EnrollRequest;
    use carosign_engine::{OperationState, TerminalOutcome};
    use carosign_runtime::OperationActor;
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    let context = carosign_engine::tests_support::initial().common.context;
    let request = EnrollRequest {
        subject: context.subject,
        authenticator: context.authenticator.id,
        target: EnrollmentLabel::new("палец").unwrap(),
        replace: None,
    };
    let mut state =
        OperationState::enrollment(context, request, MutationId::from_bytes([7; 16]).unwrap());
    state.common.phase = OperationPhase::Terminal;
    state.common.terminal = Some(TerminalOutcome::NoChange);
    let mut actor = OperationActor::new(state);
    let grant = matching_grant(actor.state());
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let outcome = futures_executor::block_on(carosignd::run::run(
        &mut actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, _| None,
    ));
    assert!(matches!(outcome, RunOutcome::Refused { .. }));
    use carosign_adapters::mutation::CompletedMutationReader;
    use carosign_persistence::model::{MutationPhase, MutationRecord, MutationTerminalOutcome};
    use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
    use carosign_persistence::store::tests_support::MemoryStore;
    use carosign_runtime::operation::hub::EventHub;
    let store = MemoryStore::default();
    let context = &actor.state().common.context;
    let mutation = actor.state().mutation().unwrap().mutation;
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([33; 16]).unwrap()),
        1,
        vec![],
        Scope {
            subject: context.subject,
            authenticator: context.authenticator.id,
        },
    )
    .unwrap();
    let record = MutationRecord {
        mutation,
        operation: context.operation_id,
        subject: context.subject,
        authenticator: context.authenticator.id,
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
    let mut hub = EventHub::new(context.operation_id);
    hub.attach(1, 0).unwrap();
    let reader = CompletedMutationReader::new(&store);
    assert!(
        futures_executor::block_on(carosignd::run::publish_observed_mutation_terminal(
            &actor, &mut hub, &Allow, &grant, &reader
        ))
        .is_err()
    );
    assert_eq!(hub.stream().last_sequence(), 0);
    let mut tx = store.begin_write().unwrap();
    tx.put(
        Table::Mutations,
        mutation.as_bytes(),
        &carosign_persistence::mutation_records::encode_mutation(&record).unwrap(),
    )
    .unwrap();
    tx.commit_durable().unwrap();
    assert_eq!(
        futures_executor::block_on(carosignd::run::publish_observed_mutation_terminal(
            &actor, &mut hub, &Allow, &grant, &reader
        )),
        Ok(carosignd::run::TerminalPublication {
            owner_outcome: Ok(TerminalOutcome::NoChange),
            disconnected: vec![]
        })
    );
    assert_eq!(hub.pop(1).unwrap().sequence, 1);
    let mut cycle_hub = EventHub::new(context.operation_id);
    cycle_hub.attach(7, 0).unwrap();
    let mut cycle_actor = OperationActor::new(actor.state().clone());
    let mut signals = Vec::new();
    let outcome = futures_executor::block_on(carosignd::run::run_observed_with_storage(
        &mut cycle_actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, _| None,
        carosignd::run::ObservedExecution {
            hub: &mut cycle_hub,
            notify: &mut |signal| signals.push(signal),
            terminal_storage: Some(&reader),
        },
    ));
    assert_eq!(outcome, RunOutcome::Terminal(TerminalOutcome::NoChange));
    assert!(signals.iter().all(Result::is_ok));
    assert_eq!(cycle_hub.pop(7).unwrap().sequence, 1);
    use revoke_after_read::RevokeAfterRead;
    let mut full_hub = EventHub::new(context.operation_id);
    full_hub.attach(42, 0).unwrap();
    for _ in 0..carosign_runtime::operation::observer::OBSERVER_QUEUE_MAX {
        full_hub
            .publish(
                OperationPhase::Running,
                carosign_runtime::operation::events::EventKind::StateChanged,
            )
            .unwrap();
    }
    let publication =
        futures_executor::block_on(carosignd::run::publish_observed_mutation_terminal(
            &actor,
            &mut full_hub,
            &RevokeAfterRead(std::sync::atomic::AtomicUsize::new(0)),
            &grant,
            &reader,
        ))
        .unwrap();
    assert_eq!(publication.owner_outcome, Err(AuthorityRefusal::Revoked));
    assert_eq!(publication.disconnected, vec![42]);
    let mut revoked_hub = EventHub::new(context.operation_id);
    revoked_hub.attach(43, 0).unwrap();
    let publication =
        futures_executor::block_on(carosignd::run::publish_observed_mutation_terminal(
            &actor,
            &mut revoked_hub,
            &RevokeAfterRead(std::sync::atomic::AtomicUsize::new(1)),
            &grant,
            &reader,
        ))
        .unwrap();
    assert_eq!(publication.owner_outcome, Err(AuthorityRefusal::Revoked));
    assert_eq!(revoked_hub.stream().last_sequence(), 1);
    let repeated = futures_executor::block_on(carosignd::run::publish_observed_mutation_terminal(
        &actor,
        &mut revoked_hub,
        &Allow,
        &grant,
        &reader,
    ))
    .unwrap();
    assert_eq!(repeated.owner_outcome, Ok(TerminalOutcome::NoChange));
    assert_eq!(revoked_hub.stream().last_sequence(), 1);
}

#[test]
fn observed_cycle_reports_disconnection_and_preserves_operation() {
    use carosign_engine::{DomainInput, EffectSpec};
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    use carosign_runtime::operation::{
        events::EventKind, hub::EventHub, observer::OBSERVER_QUEUE_MAX,
    };
    use carosign_runtime::{OperationActor, OperationMsg};
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let mut runner = EffectRunner::new(&provider, &journal, DispatchPlans::default());
    let mut actor = OperationActor::new(carosign_engine::tests_support::initial());
    let grant = matching_grant(actor.state());
    let mut hub = EventHub::new(actor.state().common.context.operation_id);
    hub.attach(1, 0).unwrap();
    for _ in 0..OBSERVER_QUEUE_MAX {
        hub.publish(OperationPhase::New, EventKind::StateChanged)
            .unwrap();
    }
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    let mut notifications = Vec::new();
    let outcome = futures_executor::block_on(carosignd::run::run_observed(
        &mut actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, _| None,
        &mut hub,
        &mut |result| notifications.push(result),
    ));
    assert!(matches!(outcome, RunOutcome::Refused { .. }));
    assert_eq!(notifications, vec![Ok(vec![1])]);
    assert!(actor.expects(EffectSpec::AcquireResources));
    assert_eq!(actor.state().common.phase, OperationPhase::WaitingLease);
}

#[test]
fn before_dispatch_rejects_foreign_scope_grant_and_boot() {
    use carosign_core::operation::context::VerificationPurpose;
    use carosign_engine::{DomainInput, EffectReceipt};
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    use carosign_runtime::{OperationActor, OperationMsg};
    use std::sync::atomic::Ordering;

    let state = carosign_engine::tests_support::initial();
    let context = &state.common.context;
    let scope = state.required_grant_scope();
    let foreign_subject = SubjectRef {
        subject: SubjectId::from_bytes([99; 16]).unwrap(),
        ..context.subject
    };
    let foreign_device = AuthenticatorId::from_bytes([88; 16]).unwrap();
    let cases = [
        (
            GrantScope::Verify {
                subject: foreign_subject,
                authenticator: context.authenticator.id,
                purpose: context.purpose,
            },
            context.owner.binding,
            context.deadline.boot(),
        ),
        (
            GrantScope::Verify {
                subject: context.subject,
                authenticator: foreign_device,
                purpose: context.purpose,
            },
            context.owner.binding,
            context.deadline.boot(),
        ),
        (
            GrantScope::Verify {
                subject: context.subject,
                authenticator: context.authenticator.id,
                purpose: VerificationPurpose::KeyOperation,
            },
            context.owner.binding,
            context.deadline.boot(),
        ),
        (
            GrantScope::Observe(context.operation_id),
            context.owner.binding,
            context.deadline.boot(),
        ),
        (
            scope.clone(),
            GrantId::from_bytes([77; 16]).unwrap(),
            context.deadline.boot(),
        ),
        (
            scope,
            context.owner.binding,
            BootId::from_bytes([66; 16]).unwrap(),
        ),
    ];
    for (scope, grant_id, boot) in cases {
        let grant = ValidatedGrant::issue_for_port(
            grant_id,
            AuthorityId::from_bytes([2; 16]).unwrap(),
            scope,
            boot,
            1000,
        );
        let authority = TrackingAuthority::default();
        let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
        let mut runner = EffectRunner::new(&provider, &Journal, DispatchPlans::default());
        let mut actor = OperationActor::new(state.clone());
        actor
            .post(OperationMsg::Domain(DomainInput::Authorized))
            .unwrap();
        let outcome = futures_executor::block_on(carosignd::run::run(
            &mut actor,
            &mut runner,
            &authority,
            &grant,
            &mut |_, _| Some(EffectReceipt::LeaseGranted),
        ));
        assert!(
            matches!(outcome, RunOutcome::Refused { reason, .. } if reason == "разрешение не соответствует операции")
        );
        assert_eq!(authority.revalidated.load(Ordering::SeqCst), 0);
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    }
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("service-{name}-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn subject() -> SubjectRef {
    SubjectRef {
        authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
        subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
    }
}

fn authenticator() -> AuthenticatorId {
    AuthenticatorId::from_bytes([3u8; 16]).expect("валид")
}

fn caller() -> CallerContext {
    CallerContext::from_transport(
        [0u8; 32],
        1000,
        TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
    )
}

use ids::Ids;

use failing_ids::FailingIds;

use clock::Clock;

use advancing_clock::AdvancingClock;

use delayed_grant::DelayedGrant;

use allow::Allow;

use provider::Provider;

use failed_transport::FailedTransport;

use revoked::Revoked;

use tracking_authority::TrackingAuthority;

use journal::Journal;

fn put(store: &RedbStore, record: &EnrollmentRecord) {
    let mut tx = store.begin_write().expect("запись");
    let bytes = carosign_persistence::enrollment_codec::encode_v3(record).expect("кодируется");
    tx.put(Table::Enrollments, record.enrollment.id.as_bytes(), &bytes)
        .expect("положено");
    let mut index = [0u8; 64];
    index[..32].copy_from_slice(&subject().to_canonical());
    index[32..48].copy_from_slice(authenticator().as_bytes());
    index[48..].copy_from_slice(record.enrollment.id.as_bytes());
    tx.put(
        Table::EnrollmentIndex,
        &index,
        record.enrollment.id.as_bytes(),
    )
    .expect("индекс");
    tx.commit_durable().expect("зафиксировано");
}

fn record() -> EnrollmentRecord {
    EnrollmentRecord {
        subject: subject(),
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([5u8; 16]).expect("валид"),
            authenticator: authenticator(),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![1, 2, 3]).expect("годится"),
        format_id: "goodix-asp".into(),
        format_version: 1,
        binding_kind: StoredBindingKind::ProviderOpaque,
    }
}

#[test]
fn ids_unavailable_refuses_before_operation_and_provider() {
    let path = temp_path("ids-unavailable");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record());
    for fail_operation in [true, false] {
        let ids = FailingIds {
            inner: Ids(std::sync::atomic::AtomicU64::new(0)),
            fail_operation,
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
        let operations = carosignd::operations::OperationRegistry::new();
        let service = Service {
            store: &store,
            authority: &Allow,
            ids: &ids,
            clock: &Clock,
            resources: &GrantAll {
                epoch: DeviceEpoch::FIRST,
            },
            resource_sets: &support::TestResourceSets,
            operations: &operations,
            provider: &provider,
            mutation: &Journal,
        };
        let result = futures_executor::block_on(service.verify(
            &caller(),
            subject(),
            authenticator(),
            DeviceEpoch::FIRST,
        ));
        assert!(
            matches!(result, Served::Refused(carosignd::answer::Reply::Refused(error))
            if error.code == carosign_protocol::carosign::common::v1::ErrorCode::ErrorUnavailable as i32)
        );
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            ids.calls.load(std::sync::atomic::Ordering::SeqCst),
            if fail_operation { 1 } else { 2 }
        );
        let mut operation = [0; 16];
        operation[15] = 1;
        assert_eq!(
            operations.phase(OperationId::from_bytes(operation).unwrap()),
            None
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn service_runs_the_operation_to_a_state() {
    let path = temp_path("run");
    let store = RedbStore::create(&path).expect("создано");
    put(&store, &record());

    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &GrantAll {
            epoch: DeviceEpoch::FIRST,
        },
        resource_sets: &support::TestResourceSets,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &journal,
    };

    let served = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    match served {
        Served::Ran {
            operation,
            deadline,
            outcome,
        } => {
            // ⚠️ The duration of the operation is from the hours of service, not from the
            // moment of response.
            assert_eq!(deadline.boot(), Clock.boot_id());
            assert_eq!(deadline.instant_ns(), 1_000 + 30_000_000_000);
            assert_eq!(operation.as_bytes()[15], 1);
            // ⚠️ The steps brought the operation to sending or waiting - but did not leave it
            // in the air. `New`.
            assert!(
                matches!(
                    outcome,
                    RunOutcome::Waiting { .. } | RunOutcome::Terminal(_)
                ),
                "неожиданный исход: {outcome:?}"
            );
            // Without a confirmed worker binding, dispatch does not occur.
            assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
        Served::Refused(reply) => panic!("ожидался запуск, а пришло {reply:?}"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn unavailable_profile_refuses_before_broker_and_provider() {
    use missing_profile::MissingProfile;
    use never_broker::NeverBroker;
    let path = temp_path("missing-resource-profile");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record());
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &NeverBroker,
        resource_sets: &MissingProfile,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &Journal,
    };
    let result = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(matches!(result, Served::Refused(_)));
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn interrupted_broker_wait_keeps_actor_and_capacity_is_checked_before_acquire() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use waiting_broker::WaitingBroker;
    let path = temp_path("interrupted-resource-wait");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record());
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let resources = WaitingBroker(std::sync::atomic::AtomicUsize::new(0));
    let operations = carosignd::operations::OperationRegistry::new();
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &resources,
        resource_sets: &support::TestResourceSets,
        operations: &operations,
        provider: &provider,
        mutation: &Journal,
    };
    let caller = caller();
    let mut context = Context::from_waker(Waker::noop());
    for number in 1..=carosignd::operations::OPERATIONS_PER_OWNER {
        let mut future =
            Box::pin(service.verify(&caller, subject(), authenticator(), DeviceEpoch::FIRST));
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        let mut bytes = [0; 16];
        bytes[15] = number as u8;
        let operation = OperationId::from_bytes(bytes).unwrap();
        assert_eq!(operations.phase(operation), None);
        drop(future);
        assert!(operations.phase(operation).is_some());
    }
    let result = futures_executor::block_on(service.verify(
        &caller,
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(
        matches!(result, Served::Refused(carosignd::answer::Reply::Refused(error)) if error.code == carosign_protocol::carosign::common::v1::ErrorCode::ErrorLimitExceeded as i32)
    );
    assert_eq!(
        resources.0.load(std::sync::atomic::Ordering::SeqCst),
        carosignd::operations::OPERATIONS_PER_OWNER
    );
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let mut first = [0; 16];
    first[15] = 1;
    assert!(!operations.has_retained_lease(OperationId::from_bytes(first).unwrap()));
    let _ = std::fs::remove_file(path);
}

#[test]
fn service_does_not_dispatch_when_deadline_expires_while_waiting_for_resources() {
    let path = temp_path("resource-deadline");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record());
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let clock = AdvancingClock(std::sync::atomic::AtomicU64::new(1_000));
    let resources = DelayedGrant(&clock);
    let operations = carosignd::operations::OperationRegistry::new();
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &clock,
        resources: &resources,
        resource_sets: &support::TestResourceSets,
        operations: &operations,
        provider: &provider,
        mutation: &Journal,
    };
    let served = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert!(
        matches!(served, Served::Ran { outcome: RunOutcome::Refused { ref reason, .. }, .. } if reason == "срок истёк при ожидании ресурсов")
    );
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let Served::Ran { operation, .. } = served else {
        panic!("нет операции")
    };
    assert!(operations.has_retained_lease(operation));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn service_rejects_foreign_lease_owner_or_mode_and_retains_the_lease() {
    use foreign_owner::ForeignOwner;

    for mismatch in [0, 1, 2] {
        let path = temp_path("foreign-lease-owner");
        let store = RedbStore::create(&path).unwrap();
        put(&store, &record());
        let ids = Ids(std::sync::atomic::AtomicU64::new(0));
        let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
        let operations = carosignd::operations::OperationRegistry::new();
        let resources = ForeignOwner { mismatch };
        let service = Service {
            store: &store,
            authority: &Allow,
            ids: &ids,
            clock: &Clock,
            resources: &resources,
            resource_sets: &support::TestResourceSets,
            operations: &operations,
            provider: &provider,
            mutation: &Journal,
        };
        let served = futures_executor::block_on(service.verify(
            &caller(),
            subject(),
            authenticator(),
            DeviceEpoch::FIRST,
        ));
        let Served::Ran {
            operation,
            outcome:
                RunOutcome::Refused {
                    phase: OperationPhase::New,
                    ..
                },
            ..
        } = served
        else {
            panic!("чужая выдача должна быть отвергнута до перехода актора")
        };
        assert!(operations.has_retained_lease(operation));
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn service_never_dispatches_with_a_stale_lease_epoch() {
    let path = temp_path("stale-lease");
    let store = RedbStore::create(&path).unwrap();
    put(&store, &record());
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let resources = GrantAll {
        epoch: DeviceEpoch::FIRST,
    };
    let operations = carosignd::operations::OperationRegistry::new();
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &resources,
        resource_sets: &support::TestResourceSets,
        operations: &operations,
        provider: &provider,
        mutation: &Journal,
    };
    let served = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::new(2).unwrap(),
    ));
    assert!(matches!(
        served,
        Served::Ran {
            outcome: RunOutcome::Refused { .. },
            ..
        }
    ));
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let Served::Ran { operation, .. } = served else {
        panic!("нет операции")
    };
    assert!(operations.has_retained_lease(operation));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn no_enrollment_refuses_before_any_operation() {
    let path = temp_path("none");
    let store = RedbStore::create(&path).expect("создано");
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let journal = Journal;
    let service = Service {
        store: &store,
        authority: &Allow,
        ids: &ids,
        clock: &Clock,
        resources: &GrantAll {
            epoch: DeviceEpoch::FIRST,
        },
        resource_sets: &support::TestResourceSets,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &journal,
    };

    match futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    )) {
        Served::Refused(_) => {}
        Served::Ran { .. } => panic!("без регистраций операция не начинается"),
    }
    // ⚠️ The provider is untouched.
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn revoked_grant_never_reaches_provider() {
    let path = temp_path("revoked");
    let store = RedbStore::create(&path).expect("создано");
    put(&store, &record());
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let service = Service {
        store: &store,
        authority: &Revoked,
        ids: &ids,
        clock: &Clock,
        resources: &GrantAll {
            epoch: DeviceEpoch::FIRST,
        },
        resource_sets: &support::TestResourceSets,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &Journal,
    };
    let _ = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn ambiguous_transport_failure_requests_cancel_instead_of_waiting_for_match() {
    use carosign_engine::{CommandRef, DomainInput, EffectReceipt, TransitionIndex, WorkerRef};
    use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
    use carosign_runtime::{
        BindingKind, EnrollmentBinding, OperationActor, OperationMsg, PermittedSet,
    };
    use std::sync::atomic::Ordering;

    let provider = FailedTransport::default();
    let mut state = carosign_engine::decide(
        &carosign_engine::tests_support::initial(),
        DomainInput::Authorized,
    )
    .unwrap()
    .next;
    let lease_effect = state.common.pending_effects[0];
    state = carosign_engine::decide(
        &state,
        DomainInput::Receipt {
            effect: lease_effect,
            receipt: EffectReceipt::LeaseGranted,
        },
    )
    .unwrap()
    .next;
    let grant_effect = state.common.pending_effects[0];
    state.common.worker = Some(WorkerRef {
        instance: WorkerInstanceId::from_bytes([1; 16]).unwrap(),
        device_epoch: state.common.context.authenticator.device_epoch,
    });
    state.common.command = Some(CommandRef {
        id: CommandId::from_bytes([2; 16]).unwrap(),
        transition_index: TransitionIndex::new(3),
    });
    let grant = matching_grant(&state);
    let mut actor = OperationActor::new(state);
    actor
        .post(OperationMsg::Domain(DomainInput::Receipt {
            effect: grant_effect,
            receipt: EffectReceipt::GrantValid,
        }))
        .unwrap();
    let permitted = PermittedSet::new(vec![EnrollmentBinding {
        enrollment: record().enrollment,
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![1],
        format_id: None,
        format_version: 0,
    }])
    .unwrap();
    let mut runner = EffectRunner::new(
        &provider,
        &Journal,
        DispatchPlans::verify(CommandPlan::Verify { permitted }),
    );
    let outcome = futures_executor::block_on(carosignd::run::run(
        &mut actor,
        &mut runner,
        &Allow,
        &grant,
        &mut |_, _| None,
    ));
    assert_eq!(provider.sent.load(Ordering::SeqCst), 1);
    assert_eq!(provider.cancelled.load(Ordering::SeqCst), 1);
    assert!(!matches!(
        outcome,
        RunOutcome::Waiting {
            phase: OperationPhase::Running
        }
    ));
}

#[test]
fn revalidates_the_original_grant_only_after_resources_are_granted() {
    use std::sync::atomic::Ordering;

    let path = temp_path("grant-boundary");
    let store = RedbStore::create(&path).expect("создано");
    put(&store, &record());
    let ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let provider = Provider(std::sync::atomic::AtomicUsize::new(0));
    let authority = TrackingAuthority::default();
    let resources = carosignd::resources::NoBroker;
    let service = Service {
        store: &store,
        authority: &authority,
        ids: &ids,
        clock: &Clock,
        resources: &resources,
        resource_sets: &support::TestResourceSets,
        operations: &carosignd::operations::OperationRegistry::new(),
        provider: &provider,
        mutation: &Journal,
    };
    let _ = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert_eq!(authority.authorized.load(Ordering::SeqCst), 1);
    assert_eq!(authority.revalidated.load(Ordering::SeqCst), 0);
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);

    let resources = GrantAll {
        epoch: DeviceEpoch::FIRST,
    };
    let second_ids = Ids(std::sync::atomic::AtomicU64::new(0));
    let second_operations = carosignd::operations::OperationRegistry::new();
    let service = Service {
        resources: &resources,
        ids: &second_ids,
        operations: &second_operations,
        ..service
    };
    let _ = futures_executor::block_on(service.verify(
        &caller(),
        subject(),
        authenticator(),
        DeviceEpoch::FIRST,
    ));
    assert_eq!(authority.authorized.load(Ordering::SeqCst), 2);
    assert_eq!(authority.revalidated.load(Ordering::SeqCst), 1);
    assert_eq!(provider.0.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_file(&path);
}
