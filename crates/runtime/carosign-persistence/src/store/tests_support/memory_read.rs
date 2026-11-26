use super::{MemoryStore, ReadTransaction, StorageError, Table, TableMap};

pub struct MemoryRead {
    pub(super) snapshot: TableMap,
}

impl ReadTransaction for MemoryRead {
    fn get(&self, table: Table, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(MemoryStore::table(&self.snapshot, table).and_then(|t| t.get(key).cloned()))
    }

    fn scan_prefix(
        &self,
        table: Table,
        prefix: &[u8],
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let mut out = Vec::new();
        if let Some(t) = MemoryStore::table(&self.snapshot, table) {
            for (k, v) in t {
                if k.starts_with(prefix) {
                    out.push((k.clone(), v.clone()));
                }
            }
        }
        Ok(out)
    }
}
