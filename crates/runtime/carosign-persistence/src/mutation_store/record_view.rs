use crate::installation_records::{checked_v4_record_as_v3, prepare_v3_record};
use crate::model::StorageError;
use crate::store::{DurableReceipt, Entry, Table, WriteTransaction};

use super::schema::{corrupt, kind};

// The adapter exists only for a single step and transfers ownership of the transaction back
// to the caller: commit here is intentionally unavailable.
pub(super) struct RecordView<'a> {
    pub(super) inner: &'a mut dyn WriteTransaction,
}

impl WriteTransaction for RecordView<'_> {
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        match kind(table) {
            Some(kind) => self.inner.put(
                table,
                key,
                &prepare_v3_record(kind, key, value).map_err(|_| corrupt())?,
            ),
            None => self.inner.put(table, key, value),
        }
    }
    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError> {
        self.inner.delete(table, key)
    }
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.inner
            .get(table, key)?
            .map(|value| match kind(table) {
                Some(kind) => checked_v4_record_as_v3(kind, key, &value).map_err(|_| corrupt()),
                None => Ok(value),
            })
            .transpose()
    }
    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        self.inner
            .scan_prefix(table, prefix)?
            .into_iter()
            .map(|(key, value)| {
                let value = match kind(table) {
                    Some(kind) => {
                        checked_v4_record_as_v3(kind, &key, &value).map_err(|_| corrupt())?
                    }
                    None => value,
                };
                Ok((key, value))
            })
            .collect()
    }
    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError> {
        Err(StorageError::Unavailable)
    }
    fn abort(self: Box<Self>) {}
}
