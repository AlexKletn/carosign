use crate::operation::events::{Event, EventKind, Progress};
use carosign_engine::OperationPhase;

use super::super::{OBSERVER_QUEUE_MAX, ObserverQueue, ObserverQueueError};

#[test]
fn slow_observer_disconnects_without_blocking_or_silently_skipping() {
    let mut queue = ObserverQueue::new(0);
    for sequence in 1..=OBSERVER_QUEUE_MAX as u64 {
        queue
            .push(Event {
                sequence,
                phase: OperationPhase::Running,
                kind: EventKind::Progress(Progress::Working),
            })
            .unwrap();
    }
    let terminal = Event {
        sequence: OBSERVER_QUEUE_MAX as u64 + 1,
        phase: OperationPhase::Terminal,
        kind: EventKind::Terminal,
    };
    assert_eq!(
        queue.push(terminal.clone()),
        Err(ObserverQueueError::Disconnect)
    );
    assert!(queue.pop().is_none());
    assert_eq!(queue.push(terminal), Err(ObserverQueueError::Disconnect));
}

#[test]
fn observer_queue_preserves_order_and_rejects_gaps() {
    let mut queue = ObserverQueue::new(5);
    let event = Event {
        sequence: 6,
        phase: OperationPhase::Running,
        kind: EventKind::StateChanged,
    };
    let wrong = Event {
        sequence: 7,
        ..event.clone()
    };
    assert_eq!(queue.push(wrong), Err(ObserverQueueError::OutOfOrder));
    queue.push(event.clone()).unwrap();
    assert_eq!(queue.pop(), Some(event));
    assert!(queue.pop().is_none());
    let mut exhausted = ObserverQueue::new(u64::MAX);
    assert_eq!(
        exhausted.push(Event {
            sequence: 0,
            phase: OperationPhase::Running,
            kind: EventKind::StateChanged
        }),
        Err(ObserverQueueError::OutOfOrder)
    );
}
