use super::{Entry, ReadTransaction, StorageError, Table, definition};

/// Borrowed access to the same image for consistency checks.
pub(crate) struct ReadTxRef<'a> {
    pub(crate) tx: &'a redb::ReadTransaction,
}

impl ReadTransaction for ReadTxRef<'_> {
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        let handle = self
            .tx
            .open_table(definition(table))
            .map_err(|_| StorageError::Corrupt {
                what: "таблица не открывается для чтения",
            })?;
        match handle.get(key).map_err(|_| StorageError::Corrupt {
            what: "чтение по ключу",
        })? {
            Some(value) => Ok(Some(value.value().to_vec())),
            None => Ok(None),
        }
    }

    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        let handle = self
            .tx
            .open_table(definition(table))
            .map_err(|_| StorageError::Corrupt {
                what: "таблица не открывается для чтения",
            })?;
        let mut out = Vec::new();
        let range = handle.range(prefix..).map_err(|_| StorageError::Corrupt {
            what: "диапазон",
        })?;
        for item in range {
            let (key, value) = item.map_err(|_| StorageError::Corrupt {
                what: "строка"
            })?;
            if !key.value().starts_with(prefix) {
                break;
            }
            out.push((key.value().to_vec(), value.value().to_vec()));
        }
        Ok(out)
    }
}
