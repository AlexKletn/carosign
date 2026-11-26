//! Reading shot and writer sequence for two repository implementations.

use carosign_persistence::redb::backend::RedbStore;
use carosign_persistence::store::{MetadataStore, Table};
use carosign_persistence::test_support::MemoryStore;

fn check_snapshot(store: &dyn MetadataStore) {
    let mut initial = store.begin_write().unwrap();
    initial.put(Table::Meta, b"a", b"before").unwrap();
    initial.commit_durable().unwrap();
    let snapshot = store.begin_read().unwrap();
    assert_eq!(
        snapshot.get(Table::Meta, b"a").unwrap().as_deref(),
        Some(&b"before"[..])
    );
    let mut write = store.begin_write().unwrap();
    write.put(Table::Meta, b"a", b"after").unwrap();
    write.put(Table::Meta, b"b", b"new").unwrap();
    write.commit_durable().unwrap();
    assert_eq!(
        snapshot.get(Table::Meta, b"a").unwrap().as_deref(),
        Some(&b"before"[..])
    );
    assert_eq!(snapshot.get(Table::Meta, b"b").unwrap(), None);
    assert_eq!(snapshot.scan_prefix(Table::Meta, b"").unwrap().len(), 1);
    let fresh = store.begin_read().unwrap();
    assert_eq!(
        fresh.get(Table::Meta, b"a").unwrap().as_deref(),
        Some(&b"after"[..])
    );
    assert_eq!(fresh.scan_prefix(Table::Meta, b"").unwrap().len(), 2);
}

#[test]
fn read_snapshot_is_stable_for_memory_and_redb() {
    check_snapshot(&MemoryStore::default());
    let path = std::env::temp_dir().join(format!("carosign-snapshot-{}.redb", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let store = RedbStore::create(&path).unwrap();
        check_snapshot(&store);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn memory_store_serializes_writers_before_reading_prior_commit() {
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    let store = Arc::new(MemoryStore::default());
    let mut first = store.begin_write().unwrap();
    first.put(Table::Meta, b"key", b"committed").unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (read_tx, read_rx) = mpsc::channel();
    let other = Arc::clone(&store);
    let worker = std::thread::spawn(move || {
        entered_tx.send(()).unwrap();
        let second = other.begin_write().unwrap();
        read_tx
            .send(second.get(Table::Meta, b"key").unwrap())
            .unwrap();
        second.abort();
    });
    entered_rx.recv().unwrap();
    assert!(read_rx.recv_timeout(Duration::from_millis(30)).is_err());
    first.commit_durable().unwrap();
    assert_eq!(
        read_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .as_deref(),
        Some(&b"committed"[..])
    );
    worker.join().unwrap();
}
