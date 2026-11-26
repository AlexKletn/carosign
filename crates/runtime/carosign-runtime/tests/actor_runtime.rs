//! Actor's end-to-end checks: management priority, one writer, failure without changing
//! state.
//!
//! ⚠️ It checks for things that you can't check by type: that the recall overtakes the
//! execution, that the control overflow is detected, and that the state after the failure
//! remains the same. The steady overflow completion is not checked here and is not connected
//! yet.

use carosign_engine::tests_support::initial;
use carosign_engine::{
    DomainInput, EffectReceipt, EffectSpec, OperationPhase, PlannedEffect, TransitionIndex,
};
use carosign_runtime::operation::actor::{OperationActor, OperationMsg, Step};
use carosign_runtime::operation::mailbox::{CONTROL_BURST, CONTROL_CAPACITY};
use carosign_runtime::operation::registry::{
    ManualClock, OPERATIONS_MAX, OperationRegistry, Owner, RegistryError,
};
use carosign_runtime::ports::TransactionId;
use carosign_runtime::{CallerContext, TimeSource};

fn actor() -> OperationActor {
    OperationActor::new(initial())
}

fn receipt(index: u64, spec: EffectSpec, receipt: EffectReceipt) -> OperationMsg {
    OperationMsg::Domain(DomainInput::Receipt {
        effect: PlannedEffect::at(spec, TransitionIndex::new(index), 0),
        receipt,
    })
}

#[test]
fn authorized_then_resources() {
    let mut actor = actor();
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .expect("влезает");
    match actor.run_once() {
        Step::Advanced { state, effects } => {
            assert_eq!(state.common.phase, OperationPhase::WaitingLease);
            assert_eq!(effects[0].spec, EffectSpec::AcquireResources);
        }
        other => panic!("ожидался переход, а пришло {other}"),
    }

    actor
        .post(receipt(
            1,
            EffectSpec::AcquireResources,
            EffectReceipt::LeaseGranted,
        ))
        .expect("влезает");
    match actor.run_once() {
        Step::Advanced { state, .. } => {
            assert_eq!(state.common.phase, OperationPhase::ReadyToDispatch);
        }
        other => panic!("ожидался переход, а пришло {other}"),
    }
}

#[test]
fn revocation_overtakes_progress() {
    // ⚠️ The review overtakes the usual queue: success will not be delivered.
    let mut actor = actor();
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    actor.run_once();
    actor
        .post(receipt(
            1,
            EffectSpec::AcquireResources,
            EffectReceipt::LeaseGranted,
        ))
        .unwrap();
    actor
        .post(receipt(
            2,
            EffectSpec::RevalidateGrant,
            EffectReceipt::GrantValid,
        ))
        .unwrap();
    actor
        .post(OperationMsg::Domain(DomainInput::Progress))
        .unwrap();
    actor.post(OperationMsg::GrantRevoked).unwrap();

    let step = actor.run_once();
    match step {
        Step::Advanced { state, .. } => {
            // The review was processed first - the operation is already in cancellation, not
            // in operation.
            assert!(state.common.cancel_requested || state.common.phase.is_terminal());
        }
        other => panic!("ожидался переход, а пришло {other}"),
    }
}

#[test]
fn control_overflow_is_reported_before_anything_else() {
    // ⚠️ Control overflows are a reason to shut down the operation, not to lose a message.
    let mut actor = actor();
    for _ in 0..CONTROL_CAPACITY {
        actor.post(OperationMsg::OwnerDetached).expect("влезает");
    }
    assert!(actor.post(OperationMsg::GrantRevoked).is_err());
    assert!(matches!(actor.run_once(), Step::Overflow));
    assert!(matches!(actor.run_once(), Step::Overflow));
}

#[test]
fn full_control_queue_is_not_itself_overflow() {
    let mut actor = actor();
    for _ in 0..CONTROL_CAPACITY {
        actor.post(OperationMsg::OwnerDetached).expect("влезает");
    }
    assert!(!matches!(actor.run_once(), Step::Overflow));
}

#[test]
fn domain_control_inputs_cannot_bypass_priority() {
    use carosign_runtime::operation::mailbox::Priority;
    for input in [
        DomainInput::OwnerRevoked,
        DomainInput::DeadlineElapsed,
        DomainInput::OwnerDetached,
        DomainInput::CancelRequested,
        DomainInput::WorkerLost,
        DomainInput::StorageUnavailable,
    ] {
        assert_eq!(OperationMsg::Domain(input).priority(), Priority::Control);
    }
}

#[test]
fn metadata_failure_overtakes_ordinary_success_receipt() {
    use carosign_runtime::operation::mailbox::Priority;

    let mut actor = actor();
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    actor
        .post(receipt(
            1,
            EffectSpec::AcquireResources,
            EffectReceipt::LeaseGranted,
        ))
        .unwrap();
    let failure = receipt(
        1,
        EffectSpec::CommitDesiredRegistryAndJournal,
        EffectReceipt::MetadataFailed,
    );
    assert_eq!(failure.priority(), Priority::Control);
    actor.post(failure).unwrap();
    // An erroneous refusal must be rejected first; the usual confirmation of the granting of
    // resources must not precede it.
    assert!(matches!(actor.run_once(), Step::Rejected(_)));
    assert_eq!(actor.state().common.phase, OperationPhase::WaitingLease);
    assert!(matches!(actor.run_once(), Step::Advanced { .. }));
    assert_eq!(actor.state().common.phase, OperationPhase::ReadyToDispatch);
}

#[test]
fn control_overflow_is_a_refusal_not_waiting() {
    let mut actor = actor();
    for _ in 0..CONTROL_CAPACITY {
        actor.post(OperationMsg::GrantRevoked).unwrap();
    }
    assert!(matches!(
        carosign_runtime::step_actor(&mut actor, OperationMsg::GrantRevoked),
        carosign_runtime::CycleStep::Refused(_)
    ));
    assert!(matches!(
        carosign_runtime::step_actor(&mut actor, OperationMsg::Domain(DomainInput::Progress)),
        carosign_runtime::CycleStep::Refused(_)
    ));
}

#[test]
fn rejected_input_leaves_state_unchanged() {
    let mut actor = actor();
    let before = actor.state().clone();
    actor
        .post(OperationMsg::Domain(DomainInput::ProviderNotMatched))
        .unwrap();
    match actor.run_once() {
        Step::Rejected(_) => {}
        other => panic!("ожидался отказ, а пришло {other}"),
    }
    assert_eq!(actor.state(), &before);
}

#[test]
fn terminal_operation_stops_accepting_work() {
    let mut actor = actor();
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    actor.run_once();
    actor
        .post(receipt(
            1,
            EffectSpec::AcquireResources,
            EffectReceipt::LeaseGranted,
        ))
        .unwrap();
    actor
        .post(receipt(
            2,
            EffectSpec::RevalidateGrant,
            EffectReceipt::GrantValid,
        ))
        .unwrap();
    actor
        .post(receipt(
            3,
            EffectSpec::SendVerify,
            EffectReceipt::SendAccepted,
        ))
        .unwrap();
    actor
        .post(OperationMsg::Domain(DomainInput::ProviderNotMatched))
        .unwrap();
    while actor.has_work() {
        actor.run_once();
    }
    assert!(actor.state().common.terminal.is_some() || actor.state().common.phase.is_terminal());
}

#[test]
fn control_burst_bounds_are_honoured() {
    // After CONTROL_BURST Control messages take the usual.
    let mut actor = actor();
    for _ in 0..CONTROL_BURST {
        actor.post(OperationMsg::Domain(DomainInput::Progress)).ok();
    }
    for _ in 0..CONTROL_BURST {
        actor.post(OperationMsg::OwnerDetached).ok();
    }
    // The first step must be management: recall goes forward.
    let step = actor.run_once();
    match step {
        Step::Advanced { state, .. } => {
            assert!(
                state.common.cancel_requested || state.common.phase.is_terminal(),
                "первым должен идти отзыв, а не ход"
            );
        }
        other => panic!("ожидался переход, а пришло {other}"),
    }
}

#[test]
fn registry_limits_and_lookup() {
    let mut registry = OperationRegistry::new();
    let owner = Owner {
        caller: CallerContext::from_transport(
            [7u8; 32],
            1000,
            TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
        ),
    };
    let actor = actor();
    let id = registry.insert(actor, owner.clone()).unwrap();
    assert!(registry.contains(id));
    assert_eq!(
        registry.insert(OperationActor::new(initial()), owner.clone()),
        Err(RegistryError::Duplicate)
    );
    let (actor, _owner) = registry.take(id).unwrap();
    registry.put(actor, owner);
    assert_eq!(registry.len(), 1);
    assert_eq!(registry.remove(id), Err(RegistryError::CleanupUnverified));
    assert_eq!(registry.len(), 1);
    assert!(registry.contains(id));
    assert!(registry.take(id).is_ok());
}

#[test]
fn terminal_phase_alone_cannot_release_registry_entry() {
    let mut state = initial();
    state.common.phase = OperationPhase::Terminal;
    let mut registry = OperationRegistry::new();
    let owner = Owner {
        caller: CallerContext::from_transport(
            [7; 32],
            1000,
            TransactionId::from_bytes([7; 16]).unwrap(),
        ),
    };
    let id = registry.insert(OperationActor::new(state), owner).unwrap();
    assert_eq!(registry.remove(id), Err(RegistryError::CleanupUnverified));
    assert!(registry.contains(id));
}

#[test]
fn checked_out_actor_keeps_its_id_and_capacity_reserved() {
    let mut registry = OperationRegistry::new();
    let owner = Owner {
        caller: CallerContext::from_transport(
            [7u8; 32],
            1000,
            TransactionId::from_bytes([7u8; 16]).unwrap(),
        ),
    };
    let id = registry.insert(actor(), owner.clone()).unwrap();
    let (checked_out, saved_owner) = registry.take(id).unwrap();
    assert_eq!(registry.len(), 1);
    assert!(registry.contains(id));
    assert_eq!(
        registry.insert(actor(), owner),
        Err(RegistryError::Duplicate)
    );
    assert!(matches!(registry.take(id), Err(RegistryError::NotFound)));
    assert_eq!(registry.remove(id), Err(RegistryError::NotFound));
    registry.put(checked_out, saved_owner);
    assert!(registry.take(id).is_ok());
}

#[test]
fn registry_is_bounded() {
    let mut registry = OperationRegistry::new();
    for i in 0..(OPERATIONS_MAX as u8) {
        let mut state = initial();
        state.common.context.operation_id =
            carosign_core::ids::OperationId::from_bytes([i + 1; 16]).expect("валид");
        let owner = Owner {
            caller: CallerContext::from_transport(
                [1u8; 32],
                1000,
                TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
            ),
        };
        registry.insert(OperationActor::new(state), owner).unwrap();
    }
    let owner = Owner {
        caller: CallerContext::from_transport(
            [1u8; 32],
            1000,
            TransactionId::from_bytes([7u8; 16]).expect("ненулевой"),
        ),
    };
    let mut duplicate = initial();
    duplicate.common.context.operation_id =
        carosign_core::ids::OperationId::from_bytes([1; 16]).unwrap();
    assert_eq!(
        registry.insert(OperationActor::new(duplicate), owner),
        Err(RegistryError::Duplicate)
    );
    let id = carosign_core::ids::OperationId::from_bytes([1; 16]).unwrap();
    let (checked_out, owner) = registry.take(id).unwrap();
    let mut new_state = initial();
    new_state.common.context.operation_id =
        carosign_core::ids::OperationId::from_bytes([200; 16]).unwrap();
    assert_eq!(
        registry.insert(OperationActor::new(new_state), owner.clone()),
        Err(RegistryError::TooMany)
    );
    registry.put(checked_out, owner);
}

#[test]
fn deadline_expiry_comes_from_the_clock() {
    let clock = ManualClock::new(initial().common.context.deadline.boot());
    let deadline = clock.now().deadline_after(100);
    assert!(!deadline.is_expired_at(clock.now()));
    clock.advance(150);
    assert!(deadline.is_expired_at(clock.now()));
}
