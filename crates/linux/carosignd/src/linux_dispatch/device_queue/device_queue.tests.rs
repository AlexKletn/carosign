use super::*;
use std::thread;

#[test]
fn queued_turns_are_bounded_ordered_and_isolated_by_authenticator() {
    let queue = DeviceQueue::new(1);
    let first = AuthenticatorId::from_bytes([1; 16]).unwrap();
    let second = AuthenticatorId::from_bytes([2; 16]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let active = queue.acquire(first, deadline, || Ok(())).unwrap();
    let other = queue.acquire(second, deadline, || Ok(())).unwrap();
    thread::scope(|scope| {
        let (admitted, received) = std::sync::mpsc::sync_channel(1);
        let shared = &queue;
        let worker = scope.spawn(move || {
            let next = shared.acquire(first, deadline, || Ok(())).unwrap();
            admitted.send(()).unwrap();
            drop(next);
        });
        while queue
            .devices
            .lock()
            .unwrap()
            .get(&first)
            .unwrap()
            .waiting
            .is_empty()
        {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert!(matches!(
            queue.acquire(first, deadline, || Ok(())),
            Err(QueueError::Limit)
        ));
        assert!(received.try_recv().is_err());
        drop(active);
        received.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap();
    });
    drop(other);
    assert!(queue.devices.lock().unwrap().is_empty());
}

#[test]
fn expired_or_revoked_waiters_release_their_places() {
    let queue = DeviceQueue::new(1);
    let id = AuthenticatorId::from_bytes([1; 16]).unwrap();
    let active = queue.acquire(id, Instant::now(), || Ok(())).unwrap();
    assert!(matches!(
        queue.acquire(id, Instant::now(), || Ok(())),
        Err(QueueError::Timeout)
    ));
    let checks = AtomicU64::new(0);
    assert!(matches!(
        queue.acquire(id, Instant::now() + Duration::from_secs(1), || {
            if checks.fetch_add(1, Ordering::Relaxed) == 0 {
                Ok(())
            } else {
                Err(QueueError::Authority(AuthorityRefusal::Revoked))
            }
        }),
        Err(QueueError::Authority(AuthorityRefusal::Revoked))
    ));
    assert!(
        queue
            .devices
            .lock()
            .unwrap()
            .get(&id)
            .unwrap()
            .waiting
            .is_empty()
    );
    drop(active);
}
