use crate::model::StorageError;
use crate::store::DurableReceipt;
use crate::store::Entry;
use crate::store::Table;
use crate::store::WriteTransaction;

pub(super) struct LostAckTx<'a>(pub(super) Box<dyn WriteTransaction + 'a>);

impl WriteTransaction for LostAckTx<'_> {
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.0.put(table, key, value)
    }
    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError> {
        self.0.delete(table, key)
    }
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.0.get(table, key)
    }
    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        self.0.scan_prefix(table, prefix)
    }
    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError> {
        self.0.commit_durable()?;
        Err(StorageError::Unavailable)
    }
    fn abort(self: Box<Self>) {
        self.0.abort();
    }
}
