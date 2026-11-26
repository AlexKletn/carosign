use super::*;
use carosign_runtime::operation::{events::EventKind, hub::EventHub};

#[test]
fn failed_publication_retains_step_and_success_is_once() {
    let state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let step = Step::Advanced {
        state: Box::new(state),
        effects: vec![],
    };
    let owner = EffectOwner::default();
    owner.accept_step(id, &step).unwrap();
    let mut closed = EventHub::new(id);
    closed
        .publish(
            carosign_engine::OperationPhase::Terminal,
            EventKind::Terminal,
        )
        .unwrap();
    assert!(matches!(
        owner.publish_step(id, &mut closed),
        Err(StepPublishError::Event(_))
    ));
    assert!(owner.step(id).is_some());
    let mut live = EventHub::new(id);
    assert!(owner.publish_step(id, &mut live).unwrap().is_empty());
    assert!(owner.step(id).is_none());
    assert!(matches!(
        owner.publish_step(id, &mut live),
        Err(StepPublishError::Missing)
    ));
}

#[test]
fn full_notification_buffer_does_not_publish_or_discard_step() {
    let state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let step = Step::Advanced {
        state: Box::new(state),
        effects: vec![],
    };
    let owner = EffectOwner::default();
    owner.accept_step(id, &step).unwrap();
    owner.notifications.lock().unwrap().insert(id, vec![7; 128]);
    let mut hub = EventHub::new(id);
    assert!(matches!(
        owner.publish_step_retained(id, &mut hub),
        Err(StepPublishError::NotificationLimit)
    ));
    assert_eq!(hub.stream().last_sequence(), 0);
    assert!(owner.step(id).is_some());
    assert_eq!(owner.take_disconnected(id), vec![7; 128]);
    assert!(owner.publish_step_retained(id, &mut hub).is_ok());
    assert_eq!(hub.stream().last_sequence(), 1);
}

#[test]
fn observer_overflow_notification_is_retained_until_taken() {
    use carosign_runtime::operation::observer::OBSERVER_QUEUE_MAX;
    let state = carosign_engine::tests_support::initial();
    let id = state.common.context.operation_id;
    let owner = EffectOwner::default();
    owner
        .accept_step(
            id,
            &Step::Advanced {
                state: Box::new(state),
                effects: vec![],
            },
        )
        .unwrap();
    let mut hub = EventHub::new(id);
    hub.attach(9, 0).unwrap();
    for _ in 0..OBSERVER_QUEUE_MAX {
        assert!(
            hub.publish(
                carosign_engine::OperationPhase::Running,
                EventKind::StateChanged
            )
            .unwrap()
            .is_empty()
        );
    }
    owner.publish_step_retained(id, &mut hub).unwrap();
    assert_eq!(owner.take_disconnected(id), vec![9]);
    assert!(owner.take_disconnected(id).is_empty());
}
