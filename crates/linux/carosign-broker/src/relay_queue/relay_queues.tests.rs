use super::*;

#[test]
fn command_capacity_does_not_consume_control_slots_and_control_has_priority() {
    let mut q = RelayQueues::default();
    for _ in 0..MAX_COMMANDS {
        q.command(vec![1]).unwrap();
    }
    assert_eq!(q.command(vec![1]), Err(QueueError::Busy));
    for _ in 0..MAX_CONTROLS {
        q.control(vec![2]).unwrap();
    }
    for _ in 0..MAX_CONTROLS {
        assert_eq!(q.next_request(), Some(vec![2]));
    }
    assert_eq!(q.next_request(), Some(vec![1]));
    q.command(vec![3]).unwrap();
}

#[test]
fn control_overflow_closes_and_discards_pending_work() {
    let mut q = RelayQueues::default();
    q.command(vec![1]).unwrap();
    for _ in 0..MAX_CONTROLS {
        q.control(vec![2]).unwrap();
    }
    assert_eq!(q.control(vec![2]), Err(QueueError::Closed));
    assert_eq!(q.next_request(), None);
    assert_eq!(q.command(vec![3]), Err(QueueError::Closed));
}

#[test]
fn event_overflow_closes_instead_of_losing_terminal() {
    let mut q = RelayQueues::default();
    q.command(vec![1]).unwrap();
    for _ in 0..MAX_EVENTS {
        q.event(vec![2]).unwrap();
    }
    assert_eq!(q.event(vec![3]), Err(QueueError::Closed));
    assert_eq!(q.next_request(), None);
    assert_eq!(q.next_event(), None);
    assert_eq!(q.event(vec![3]), Err(QueueError::Closed));
}

#[test]
fn outgoing_order_and_frame_limits() {
    let mut q = RelayQueues::default();
    assert_eq!(
        q.command(vec![0; MAX_FRAME + 1]),
        Err(QueueError::InvalidFrame)
    );
    q.event(vec![1]).unwrap();
    q.event(vec![2]).unwrap();
    assert_eq!(q.next_event(), Some(vec![1]));
    assert_eq!(q.next_event(), Some(vec![2]));
    assert_eq!(q.event(vec![]), Err(QueueError::InvalidFrame));
    assert_eq!(q.event(vec![1]), Err(QueueError::Closed));
}
