use super::*;
use crate::operation::observer::OBSERVER_QUEUE_MAX;

fn awaiting_delivery() -> (
    carosign_engine::OperationState,
    carosign_engine::PlannedEffect,
) {
    use carosign_engine::{EffectSpec, PlannedEffect, TerminalOutcome, TransitionIndex};
    let mut state = carosign_engine::tests_support::initial();
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(TerminalOutcome::FactorAccepted);
    state.common.lease = None;
    let effect = PlannedEffect::at(EffectSpec::DeliverOutcome, TransitionIndex::new(3), 0);
    state.common.pending_effects = vec![effect];
    (state, effect)
}

#[test]
fn reserved_terminal_is_invisible_until_sent_and_recorded() {
    use carosign_engine::transition::VerificationDelivery;
    let (mut state, effect) = awaiting_delivery();
    let mut hub = EventHub::new(state.common.context.operation_id);
    hub.attach(1, 0).unwrap();
    let reservation = hub.reserve_positive_terminal(&state, effect).unwrap();
    assert_eq!(reservation.sequence(), 1);
    assert_eq!(hub.stream().last_sequence(), 0);
    assert!(hub.pop(1).is_none());
    assert_eq!(
        hub.publish(OperationPhase::Running, EventKind::StateChanged),
        Err(EventError::Reserved)
    );
    assert!(matches!(
        hub.reserve_positive_terminal(&state, effect),
        Err(ReservationError::Busy)
    ));
    hub.begin_reserved_send(&reservation).unwrap();
    hub.finish_reserved_send(&reservation, ReservedSendOutcome::NotSent)
        .unwrap();
    hub.begin_reserved_send(&reservation).unwrap();
    hub.finish_reserved_send(&reservation, ReservedSendOutcome::Sent)
        .unwrap();
    assert_eq!(
        hub.publish_reserved_terminal(&reservation, &state),
        Err(ReservationError::ContextMismatch)
    );
    state.common.phase = OperationPhase::Terminal;
    state.common.pending_effects.clear();
    let carosign_engine::OperationBody::Verification(ref mut verification) = state.body else {
        unreachable!()
    };
    verification.delivery = VerificationDelivery::Sent;
    verification.evidence = None;
    hub.publish_reserved_terminal(&reservation, &state).unwrap();
    assert_eq!(hub.pop(1).unwrap().sequence, reservation.sequence());
    assert_eq!(hub.stream().last_sequence(), 1);
    assert_eq!(
        hub.publish_reserved_terminal(&reservation, &state),
        Err(ReservationError::ContextMismatch)
    );
}

#[test]
fn uncertain_send_cannot_be_retried_or_cancelled() {
    let (state, effect) = awaiting_delivery();
    let mut hub = EventHub::new(state.common.context.operation_id);
    let reservation = hub.reserve_positive_terminal(&state, effect).unwrap();
    hub.begin_reserved_send(&reservation).unwrap();
    hub.finish_reserved_send(&reservation, ReservedSendOutcome::Unknown)
        .unwrap();
    assert_eq!(
        hub.begin_reserved_send(&reservation),
        Err(ReservationError::Busy)
    );
    assert_eq!(
        hub.cancel_reserved_terminal(&reservation),
        Err(ReservationError::Busy)
    );
    assert!(hub.pop(1).is_none());
    assert_eq!(hub.stream().last_sequence(), 0);
}

#[test]
fn foreign_hub_cannot_consume_reservation_even_with_same_operation() {
    let (state, effect) = awaiting_delivery();
    let mut first = EventHub::new(state.common.context.operation_id);
    let mut second = EventHub::new(state.common.context.operation_id);
    let reservation = first.reserve_positive_terminal(&state, effect).unwrap();
    assert_eq!(
        second.begin_reserved_send(&reservation),
        Err(ReservationError::ContextMismatch)
    );
    first.cancel_reserved_terminal(&reservation).unwrap();
    let replacement = first.reserve_positive_terminal(&state, effect).unwrap();
    assert_eq!(replacement.sequence(), 1);
    assert_eq!(
        first.begin_reserved_send(&reservation),
        Err(ReservationError::ContextMismatch)
    );
}

#[test]
fn slow_observer_is_removed_without_affecting_other_observers_or_history() {
    let mut hub = EventHub::new(OperationId::from_bytes([1; 16]).unwrap());
    hub.attach(1, 0).unwrap();
    hub.attach(2, 0).unwrap();
    for sequence in 1..=OBSERVER_QUEUE_MAX as u64 {
        assert!(
            hub.publish(OperationPhase::Running, EventKind::StateChanged)
                .unwrap()
                .is_empty()
        );
        assert_eq!(hub.pop(2).unwrap().sequence, sequence);
    }
    assert_eq!(
        hub.publish(OperationPhase::Finalizing, EventKind::StateChanged)
            .unwrap(),
        vec![1]
    );
    assert_eq!(hub.pop(2).unwrap().sequence, OBSERVER_QUEUE_MAX as u64 + 1);
    assert!(hub.pop(1).is_none());
    assert_eq!(hub.stream().last_sequence(), OBSERVER_QUEUE_MAX as u64 + 1);
    assert_eq!(hub.attach(1, 0), Err(AttachError::CatchupRequired));
    hub.attach(1, hub.stream().last_sequence()).unwrap();
    assert!(hub.detach(1));
    assert!(!hub.detach(1));
}

#[test]
fn subscriptions_are_bounded_and_unique() {
    let mut hub = EventHub::new(OperationId::from_bytes([1; 16]).unwrap());
    for id in 0..OBSERVERS_MAX as u32 {
        hub.attach(id, 0).unwrap();
    }
    assert_eq!(hub.attach(0, 0), Err(AttachError::Duplicate));
    assert_eq!(hub.attach(99, 0), Err(AttachError::Limit));
}

#[test]
fn actor_phase_is_published_without_losing_its_effects() {
    use crate::operation::actor::{OperationActor, OperationMsg, Step};
    use carosign_engine::{DomainInput, EffectSpec};
    let state = carosign_engine::tests_support::initial();
    let operation = state.common.context.operation_id;
    let mut actor = OperationActor::new(state);
    let mut hub = EventHub::new(operation);
    hub.attach(1, 0).unwrap();
    actor
        .post(OperationMsg::Domain(DomainInput::Authorized))
        .unwrap();
    let result = hub.step_actor(&mut actor).unwrap();
    assert!(
        matches!(result.step, Step::Advanced { effects, .. } if effects.iter().any(|effect| effect.spec == EffectSpec::AcquireResources))
    );
    assert!(result.notification.unwrap().is_empty());
    let event = hub.pop(1).unwrap();
    assert_eq!(event.sequence, 1);
    assert_eq!(event.phase, OperationPhase::WaitingLease);
    assert_eq!(event.kind, EventKind::StateChanged);
    let mut foreign = EventHub::new(OperationId::from_bytes([99; 16]).unwrap());
    assert!(matches!(
        foreign.step_actor(&mut actor),
        Err(AttachError::WrongOperation)
    ));
}
