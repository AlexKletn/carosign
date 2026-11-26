use super::*;
use carosign_core::ids::OperationId;
use carosign_engine::OperationPhase;
use carosign_runtime::operation::{
    events::{EventKind, HISTORY_MAX},
    observer::OBSERVERS_MAX,
};

#[test]
fn replay_gap_and_live_queue_share_one_cursor() {
    let hub = Arc::new(Mutex::new(EventHub::new(
        OperationId::from_bytes([1; 16]).unwrap(),
    )));
    for _ in 0..=HISTORY_MAX {
        hub.lock()
            .unwrap()
            .publish(OperationPhase::Running, EventKind::StateChanged)
            .unwrap();
    }
    let latest = hub.lock().unwrap().stream().last_sequence();
    assert!(matches!(
        OperationSubscription::attach(hub.clone(), latest - 1, 0, OBSERVERS_MAX),
        Err(AttachError::CatchupRequired)
    ));
    let (subscription, replay) =
        OperationSubscription::attach(hub.clone(), latest, 0, OBSERVERS_MAX).unwrap();
    assert!(replay.is_none());
    let (recent, replay) =
        OperationSubscription::attach(hub.clone(), latest, latest - 2, OBSERVERS_MAX).unwrap();
    assert_eq!(
        replay
            .unwrap()
            .into_iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![latest - 1, latest]
    );
    hub.lock()
        .unwrap()
        .publish(OperationPhase::Running, EventKind::StateChanged)
        .unwrap();
    assert_eq!(subscription.pop().unwrap().unwrap().sequence, latest + 1);
    assert_eq!(recent.pop().unwrap().unwrap().sequence, latest + 1);

    let mut remaining = Vec::new();
    for _ in 2..OBSERVERS_MAX {
        remaining.push(
            OperationSubscription::attach(hub.clone(), latest + 1, latest + 1, OBSERVERS_MAX)
                .unwrap(),
        );
    }
    assert!(matches!(
        OperationSubscription::attach(hub.clone(), latest + 1, latest + 1, OBSERVERS_MAX),
        Err(AttachError::Limit)
    ));
    drop(subscription);
    assert!(OperationSubscription::attach(hub, latest + 1, latest + 1, OBSERVERS_MAX).is_ok());
}

#[test]
fn configured_limit_rejects_second_observer_until_slot_is_released() {
    let hub = Arc::new(Mutex::new(EventHub::new(
        OperationId::from_bytes([2; 16]).unwrap(),
    )));
    let (first, _) = OperationSubscription::attach(hub.clone(), 0, 0, 1).unwrap();
    assert!(matches!(
        OperationSubscription::attach(hub.clone(), 0, 0, 1),
        Err(AttachError::Limit)
    ));
    drop(first);
    assert!(OperationSubscription::attach(hub, 0, 0, 1).is_ok());
}
