use crate::model::StorageError;
use crate::store::DurableReceipt;
use crate::store::Entry;
use crate::store::Table;
use crate::store::WriteTransaction;

pub(super) struct Transaction<'a> {
    pub(super) inner: Box<dyn WriteTransaction + 'a>,
    pub(super) lose_ack: bool,
}

impl WriteTransaction for Transaction<'_> {
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.inner.get(table, key)
    }
    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        self.inner.scan_prefix(table, prefix)
    }
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.inner.put(table, key, value)
    }
    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError> {
        self.inner.delete(table, key)
    }
    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError> {
        let receipt = self.inner.commit_durable()?;
        if self.lose_ack {
            Err(StorageError::Unavailable)
        } else {
            Ok(receipt)
        }
    }
    fn abort(self: Box<Self>) {
        self.inner.abort();
    }
}
