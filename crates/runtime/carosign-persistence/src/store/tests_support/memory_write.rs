use super::{BTreeMap, DurableReceipt, MemoryStore, StorageError, Table, WriteTransaction};

pub struct MemoryWrite<'a> {
    pub(super) store: &'a MemoryStore,
    pub(super) _writer: std::sync::MutexGuard<'a, ()>,
    pub(super) staged: BTreeMap<&'static str, BTreeMap<Vec<u8>, Option<Vec<u8>>>>,
}

impl WriteTransaction for MemoryWrite<'_> {
    fn put(&mut self, table: Table, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.staged
            .entry(table.name())
            .or_default()
            .insert(key.to_vec(), Some(value.to_vec()));
        Ok(())
    }

    fn delete(&mut self, table: Table, key: &[u8]) -> Result<(), StorageError> {
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
        let map = self.store.data.lock().map_err(|_| StorageError::Backend)?;
        Ok(MemoryStore::table(&map, table).and_then(|t| t.get(key).cloned()))
    }

    fn scan_prefix(
        &self,
        table: Table,
        prefix: &[u8],
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let map = self.store.data.lock().map_err(|_| StorageError::Backend)?;
        let mut merged = map.get(table.name()).cloned().unwrap_or_default();
        if let Some(stage) = self.staged.get(table.name()) {
            for (k, v) in stage {
                match v {
                    Some(value) => {
                        merged.insert(k.clone(), value.clone());
                    }
                    None => {
                        merged.remove(k);
                    }
                }
            }
        }
        let mut out = Vec::new();
        for (k, v) in merged {
            if k.starts_with(prefix) {
                out.push((k, v));
            }
        }
        Ok(out)
    }

    fn commit_durable(self: Box<Self>) -> Result<DurableReceipt, StorageError> {
        let mut fail = self
            .store
            .fail_next_commit
            .lock()
            .map_err(|_| StorageError::Backend)?;
        if *fail {
            *fail = false;
            return Err(StorageError::Unavailable);
        }
        drop(fail);
        let mut map = self.store.data.lock().map_err(|_| StorageError::Backend)?;
        for (table, stage) in self.staged {
            let target = map.entry(table).or_default();
            for (k, v) in stage {
                match v {
                    Some(value) => {
                        target.insert(k, value);
                    }
                    None => {
                        target.remove(&k);
                    }
                }
            }
        }
        Ok(DurableReceipt::confirmed_by_backend())
    }

    fn abort(self: Box<Self>) {}
}
