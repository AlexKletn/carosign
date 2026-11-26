use super::{DurableReceipt, Entry, StorageError, Table};

/// Record.
///
/// ⚠️ The transaction lives no longer than one step of change. If the step is broken, the
/// data should not appear recorded.
pub trait WriteTransaction {
    /// Puts or replaces a value.
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError>;
    /// Deletes the value if there is.
    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError>;
    /// It reads inside the same transaction.
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    /// Reads the prefix inside the same transaction.
    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError>;
    /// Fixing and **waiting* for the disc.
    ///
    /// ⚠️ `StorageError::Unavailable` It does not prove either success or lack of record:
    /// after ambiguous failure, you need to read a steady state.
    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError>;
    /// Rejects the transaction without modification.
    fn abort(self: Box<Self>);
}
