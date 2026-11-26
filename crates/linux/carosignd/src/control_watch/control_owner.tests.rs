use super::super::SignalError;
use super::*;
use carosign_adapters::clock::StepClock;
use carosign_core::authorization::ValidatedGrant;
use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, decide};
use carosign_runtime::OperationActor;
use carosign_runtime::operation::actor::Step;
use carosign_runtime::ports::{CallerContext, TransactionId};

fn fixture() -> (Arc<OperationRegistry>, OperationId, Arc<StepClock>) {
    let mut state = decide(
        &carosign_engine::tests_support::initial(),
        DomainInput::Authorized,
    )
    .unwrap()
    .next;
    for receipt in [EffectReceipt::LeaseGranted, EffectReceipt::GrantValid] {
        let effect = state.common.pending_effects[0];
        state = decide(&state, DomainInput::Receipt { effect, receipt })
            .unwrap()
            .next;
    }
    let id = state.common.context.operation_id;
    let clock = Arc::new(StepClock::new(state.common.context.deadline.boot()));
    let grant = ValidatedGrant::issue_for_port(
        state.common.context.owner.binding,
        state.common.context.owner.authority,
        state.required_grant_scope(),
        state.common.context.deadline.boot(),
        state.common.context.deadline.instant_ns(),
    );
    let registry = Arc::new(OperationRegistry::new());
    let caller =
        CallerContext::from_transport([1; 32], 1000, TransactionId::from_bytes([1; 16]).unwrap());
    drop(
        registry
            .reserve(OperationActor::new(state), grant, &caller)
            .unwrap(),
    );
    (registry, id, clock)
}

#[test]
fn source_disconnect_revokes_and_abandoned_claim_returns_effects() {
    let (registry, id, clock) = fixture();
    let owner = Arc::new(ControlOwner::default());
    let (watch, source) = owner
        .start(registry, id, clock, Duration::from_secs(1))
        .unwrap();
    assert!(owner.claim(id).is_none());
    drop(source);
    watch.wait();
    {
        let claim = owner.claim(id).unwrap();
        assert!(
            matches!(claim.result(), WatchResult::Step(Step::Advanced { effects, .. }) if effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult))
        );
    }
    let claim = owner.claim(id).expect("эффекты возвращены в ящик");
    claim.acknowledge();
    assert!(owner.claim(id).is_none());
}

#[test]
fn client_cancellation_acknowledges_only_after_control_step_is_retained() {
    let (registry, id, clock) = fixture();
    let owner = Arc::new(ControlOwner::default());
    let (watch, source) = owner
        .start(registry.clone(), id, clock, Duration::from_secs(2))
        .unwrap();
    source.cancel_and_wait(Duration::from_secs(1)).unwrap();
    let claim = owner
        .claim(id)
        .expect("подтверждённая отмена должна сохранить шаг");
    assert!(
        matches!(claim.result(), WatchResult::Step(Step::Advanced { state, .. }) if state.common.cancel_requested)
    );
    claim.acknowledge();
    watch.wait();
}

#[test]
fn policy_file_change_revokes_running_operation() {
    let _lock = crate::policy_test_lock();
    let (registry, id, clock) = fixture();
    let dir = std::path::Path::new("/dev/shm")
        .join(format!("carosign-control-policy-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("policy");
    std::fs::write(&path, b"before").unwrap();
    let policy = carosign_linux::policy::PolicyChangeWatch::new(&path).unwrap();
    let owner = Arc::new(ControlOwner::default());
    let (watch, _source) = owner
        .start_observing(registry, id, clock, Duration::from_secs(2), Some(policy))
        .unwrap();
    std::fs::write(&path, b"after").unwrap();
    watch.wait();
    let claim = owner.claim(id).expect("отзыв должен быть сохранён");
    assert!(
        matches!(claim.result(), WatchResult::Step(Step::Advanced { effects, .. }) if effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult))
    );
    claim.acknowledge();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn busy_actor_receives_deferred_revocation_and_stop_is_bounded() {
    let (registry, id, clock) = fixture();
    let owner = Arc::new(ControlOwner::default());
    let mut guard = registry.checkout(id).unwrap();
    guard
        .operation_mut()
        .actor
        .post(carosign_runtime::OperationMsg::Domain(
            DomainInput::ProviderNoUsableSample,
        ))
        .unwrap();
    let (watch, source) = owner
        .start(Arc::clone(&registry), id, clock, Duration::from_secs(1))
        .unwrap();
    source.revoke().unwrap();
    watch.wait();
    assert_eq!(source.revoke(), Err(SignalError::Closed));
    assert!(matches!(
        owner.claim(id).unwrap().result(),
        WatchResult::Deferred
    ));
    drop(guard);
    let mut guard = registry.checkout(id).unwrap();
    assert!(
        matches!(guard.operation_mut().actor.run_once(), Step::Advanced { ref effects, .. } if effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult))
    );
    drop(guard);
    owner.claim(id).unwrap().acknowledge();
    let (registry, id, clock) = fixture();
    let (watch, _source) = owner
        .start(registry, id, clock, Duration::from_secs(1))
        .unwrap();
    let before = Instant::now();
    watch.stop();
    assert!(before.elapsed() < Duration::from_secs(1));
    assert!(owner.claim(id).is_some());
}

#[test]
fn deadline_and_lifetime_expiry_fail_closed_and_capacity_is_bounded() {
    let (registry, id, clock) = fixture();
    let owner = Arc::new(ControlOwner::default());
    let (watch, _source) = owner
        .start(
            Arc::clone(&registry),
            id,
            clock.clone(),
            Duration::from_secs(1),
        )
        .unwrap();
    clock.advance(u64::MAX);
    watch.wait();
    assert!(
        matches!(owner.claim(id).unwrap().result(), WatchResult::Step(Step::Advanced { effects, .. }) if effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult))
    );
    owner.claim(id).unwrap().acknowledge();
    let (registry, id, clock) = fixture();
    let (watch, _source) = owner
        .start(registry, id, clock, Duration::from_millis(30))
        .unwrap();
    watch.wait();
    assert!(owner.claim(id).is_some());
    let full = Arc::new(ControlOwner::default());
    for n in 1..=WATCH_MAX {
        let mut bytes = [0; 16];
        bytes[0] = n as u8;
        full.slots
            .lock()
            .unwrap()
            .insert(OperationId::from_bytes(bytes).unwrap(), Slot::Running);
    }
    let (registry, id, clock) = fixture();
    assert!(matches!(
        full.start(registry, id, clock, Duration::from_secs(1)),
        Err(StartError::Limit)
    ));
}

#[test]
fn clock_failure_suppresses_operation_instead_of_losing_watch_result() {
    struct FailedClock(carosign_core::ids::BootId);
    impl TimeSource for FailedClock {
        fn now(&self) -> carosign_core::operation::context::BootInstant {
            panic!("clock unavailable")
        }
        fn boot_id(&self) -> carosign_core::ids::BootId {
            self.0
        }
    }
    let (registry, id, clock) = fixture();
    let owner = Arc::new(ControlOwner::default());
    let failed = Arc::new(FailedClock(clock.boot_id()));
    let (watch, _source) = owner
        .start(registry, id, failed, Duration::from_secs(1))
        .unwrap();
    watch.wait();
    assert!(
        matches!(owner.claim(id).unwrap().result(), WatchResult::Step(Step::Advanced { effects, .. }) if effects.iter().any(|effect| effect.spec == EffectSpec::SuppressResult))
    );
}
