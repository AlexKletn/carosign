use super::{DurableReceipt, Entry, StorageError, Table, WriteTransaction, WriteTx};

/// A transaction created by a vetted owner of the v5 primary scheme.
pub(crate) struct GuardedWriteTx<'a>(pub(in super::super) WriteTx<'a>);

impl GuardedWriteTx<'_> {
    /// Access for installations not included in the general listing of tables.
    pub(crate) fn installation_transaction(&self) -> Result<&redb::WriteTransaction, StorageError> {
        self.0.tx.as_ref().ok_or(StorageError::Unavailable)
    }
}

impl WriteTransaction for GuardedWriteTx<'_> {
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
        Box::new(self.0).commit_durable()
    }

    fn abort(self: Box<Self>) {
        Box::new(self.0).abort()
    }
}
