use super::{Entry, StorageError, Table};

/// Reading.
///
/// ⚠️ Not. `async`: the transaction should not cross `await`While reading, we're holding the
/// picture.
pub trait ReadTransaction {
    /// He's reading key.
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError>;
    /// It reads by prefix (for indexes).
    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError>;
}
