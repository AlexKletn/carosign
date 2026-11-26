use crate::installation_records::checked_v4_record_as_v3;
use crate::model::StorageError;
use crate::store::{Entry, ReadTransaction, Table};

use super::schema::{corrupt, kind};

pub(super) struct ReadView<'a> {
    pub(super) inner: &'a dyn ReadTransaction,
}

impl ReadTransaction for ReadView<'_> {
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
}
