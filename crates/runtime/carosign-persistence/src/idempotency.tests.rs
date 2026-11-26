use super::*;
use crate::store::{MetadataStore, tests_support::MemoryStore};
use carosign_core::ids::{IdempotencyKey, MutationId, OperationId};

fn record() -> IdempotencyRecord {
    IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: Some(MutationId::from_bytes([5; 16]).unwrap()),
    }
}

#[test]
fn same_request_returns_original_and_other_request_conflicts() {
    let store = MemoryStore::default();
    let original = record();
    let mut tx = store.begin_write().unwrap();
    assert_eq!(stage(&mut *tx, &original), Ok(Reservation::Staged));
    tx.commit_durable().unwrap();
    let mut retry = original.clone();
    retry.operation = OperationId::from_bytes([6; 16]).unwrap();
    retry.mutation = Some(MutationId::from_bytes([7; 16]).unwrap());
    let mut tx = store.begin_write().unwrap();
    assert_eq!(
        stage(&mut *tx, &retry),
        Ok(Reservation::Existing(original.clone()))
    );
    retry.request_digest[0] ^= 1;
    assert_eq!(stage(&mut *tx, &retry), Err(ReservationError::Conflict));
    retry.request_digest = original.request_digest;
    retry.mutation = None;
    assert!(matches!(
        stage(&mut *tx, &retry),
        Err(ReservationError::Storage(StorageError::Corrupt { .. }))
    ));
    tx.abort();
    let mut foreign = original.clone();
    foreign.caller_digest[0] ^= 1;
    let mut tx = store.begin_write().unwrap();
    assert_eq!(stage(&mut *tx, &foreign), Ok(Reservation::Staged));
    tx.abort();
}

#[test]
fn abort_or_failed_commit_does_not_persist_staged_rows_in_memory_store() {
    let store = MemoryStore::default();
    let candidate = record();
    let key = crate::redb::codec::idempotency_key(&candidate.caller_digest, &candidate.key);
    for fail_commit in [false, true] {
        let mut tx = store.begin_write().unwrap();
        assert_eq!(stage(&mut *tx, &candidate), Ok(Reservation::Staged));
        tx.put(Table::MutationScopeLock, b"scope", b"mutation")
            .unwrap();
        if fail_commit {
            *store.fail_next_commit.lock().unwrap() = true;
            assert_eq!(tx.commit_durable(), Err(StorageError::Unavailable));
        } else {
            tx.abort();
        }
        let read = store.begin_read().unwrap();
        assert!(read.get(Table::Idempotency, &key).unwrap().is_none());
        assert!(
            read.get(Table::MutationScopeLock, b"scope")
                .unwrap()
                .is_none()
        );
    }
}
