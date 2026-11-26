use super::{Entry, ReadTransaction, ReadTxRef, StorageError, Table};

/// Read inside one red version of the database.
pub(crate) struct ReadTx {
    pub(crate) tx: redb::ReadTransaction,
}

impl ReadTransaction for ReadTx {
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        ReadTxRef { tx: &self.tx }.get(table, key)
    }

    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        ReadTxRef { tx: &self.tx }.scan_prefix(table, prefix)
    }
}
