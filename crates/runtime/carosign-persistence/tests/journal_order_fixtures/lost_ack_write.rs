use carosign_persistence::model::StorageError;
use carosign_persistence::store::DurableReceipt;
use carosign_persistence::store::Entry;
use carosign_persistence::store::Table;
use carosign_persistence::store::WriteTransaction;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

pub(super) struct LostAckWrite<'a> {
    pub(super) inner: Box<dyn WriteTransaction + 'a>,
    pub(super) lose_ack: &'a AtomicBool,
}

impl WriteTransaction for LostAckWrite<'_> {
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.inner.put(table, key, value)
    }
    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError> {
        self.inner.delete(table, key)
    }
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.inner.get(table, key)
    }
    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        self.inner.scan_prefix(table, prefix)
    }
    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError> {
        let receipt = self.inner.commit_durable()?;
        if self.lose_ack.swap(false, Ordering::SeqCst) {
            Err(StorageError::Unavailable)
        } else {
            Ok(receipt)
        }
    }
    fn abort(self: Box<Self>) {
        self.inner.abort();
    }
}
