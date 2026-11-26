use super::{
    DurableReceipt, Entry, InitialFenceStore, ReadableTable, SCHEMA_KEY, StorageError, Table,
    WriteTransaction, definition,
};

/// A record that becomes visible only when fixed.
///
/// ⚠️ Everything that is supposed to be in `redb`- Transactions; `staged` only store our copy
/// for reading inside the transaction (y) `redb` It is possible to read what is recorded in
/// the same transaction, but it is cheaper to bypass it with your vocabulary than relying on
/// the library version.
pub(in super::super) struct WriteTx<'a> {
    pub(in super::super) tx: Option<redb::WriteTransaction>,
    pub(in super::super) guard: Option<&'a InitialFenceStore>,
    pub(in super::super) staged: std::collections::BTreeMap<
        &'static str,
        std::collections::BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    >,
}

impl WriteTransaction for WriteTx<'_> {
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        if table == Table::Meta && key == SCHEMA_KEY {
            return Err(StorageError::Unavailable);
        }
        let tx = self.tx.as_ref().ok_or(StorageError::Unavailable)?;
        {
            let mut handle = tx
                .open_table(definition(table))
                .map_err(|_| StorageError::Unavailable)?;
            handle
                .insert(key, value)
                .map_err(|_| StorageError::Unavailable)?;
        }
        self.staged
            .entry(table.name())
            .or_default()
            .insert(key.to_vec(), Some(value.to_vec()));
        Ok(())
    }

    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError> {
        if table == Table::Meta && key == SCHEMA_KEY {
            return Err(StorageError::Unavailable);
        }
        let tx = self.tx.as_ref().ok_or(StorageError::Unavailable)?;
        {
            let mut handle = tx
                .open_table(definition(table))
                .map_err(|_| StorageError::Unavailable)?;
            handle.remove(key).map_err(|_| StorageError::Unavailable)?;
        }
        self.staged
            .entry(table.name())
            .or_default()
            .insert(key.to_vec(), None);
        Ok(())
    }

    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        if let Some(value) = self
            .staged
            .get(table.name())
            .and_then(|stage| stage.get(key))
        {
            return Ok(value.clone());
        }
        let tx = self.tx.as_ref().ok_or(StorageError::Unavailable)?;
        let handle = tx
            .open_table(definition(table))
            .map_err(|_| StorageError::Unavailable)?;
        match handle.get(key).map_err(|_| StorageError::Unavailable)? {
            Some(value) => Ok(Some(value.value().to_vec())),
            None => Ok(None),
        }
    }

    fn scan_prefix(&self, table: Table, prefix: &[u8]) -> Result<Vec<Entry>, StorageError> {
        let tx = self.tx.as_ref().ok_or(StorageError::Unavailable)?;
        let handle = tx
            .open_table(definition(table))
            .map_err(|_| StorageError::Unavailable)?;
        let mut merged: std::collections::BTreeMap<Vec<u8>, Vec<u8>> =
            std::collections::BTreeMap::new();
        let range = handle.range(prefix..).map_err(|_| StorageError::Corrupt {
            what: "диапазон",
        })?;
        for item in range {
            let (key, value) = item.map_err(|_| StorageError::Unavailable)?;
            if !key.value().starts_with(prefix) {
                break;
            }
            merged.insert(key.value().to_vec(), value.value().to_vec());
        }
        if let Some(stage) = self.staged.get(table.name()) {
            for (key, value) in stage {
                if !key.starts_with(prefix) {
                    continue;
                }
                match value {
                    Some(value) => {
                        merged.insert(key.clone(), value.clone());
                    }
                    None => {
                        merged.remove(key);
                    }
                }
            }
        }
        Ok(merged.into_iter().collect())
    }

    /// Fixing and **waiting* for the disc.
    ///
    /// `Durability::Immediate` waits for the write to reach storage through `fsync`. A
    /// commit using `Eventual` cannot produce a durable receipt.
    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError> {
        if let Some(guard) = self.guard {
            guard.check_identity()?;
        }
        let tx = self.tx.ok_or(StorageError::Unavailable)?;
        tx.commit().map_err(|_| StorageError::Unavailable)?;
        if let Some(guard) = self.guard {
            guard.check_identity()?;
        }
        Ok(DurableReceipt::confirmed_by_backend())
    }

    fn abort(self: Box<Self>) {
        if let Some(tx) = self.tx {
            let _ = tx.abort();
        }
    }
}
