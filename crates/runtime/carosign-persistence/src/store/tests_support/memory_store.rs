use super::{
    BTreeMap, MemoryRead, MemoryWrite, MetadataStore, Mutex, ReadTransaction, StorageError, Table,
    TableMap, WriteTransaction,
};

/// A fake storage in the memory for checks.
///
/// ⚠️ Resistance here is **stated**, not tested: a forgery is good for log logic, but not for
/// power failure tests.
#[derive(Default)]
pub struct MemoryStore {
    pub(super) data: Mutex<TableMap>,
    writer: Mutex<()>,
    /// Next Next post `commit_durable` That'll get you back.
    pub fail_next_commit: Mutex<bool>,
    /// Next Next post `begin_write` That'll get you back.
    pub fail_next_write: Mutex<bool>,
}

impl MemoryStore {
    pub(super) fn table<'a>(
        map: &'a BTreeMap<&'static str, BTreeMap<Vec<u8>, Vec<u8>>>,
        table: Table,
    ) -> Option<&'a BTreeMap<Vec<u8>, Vec<u8>>> {
        map.get(table.name())
    }
}

impl MetadataStore for MemoryStore {
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError> {
        let snapshot = self.data.lock().map_err(|_| StorageError::Backend)?.clone();
        Ok(Box::new(MemoryRead { snapshot }))
    }

    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError> {
        let mut fail = self
            .fail_next_write
            .lock()
            .map_err(|_| StorageError::Backend)?;
        if *fail {
            *fail = false;
            return Err(StorageError::Unavailable);
        }
        drop(fail);
        let writer = self.writer.lock().map_err(|_| StorageError::Backend)?;
        Ok(Box::new(MemoryWrite {
            store: self,
            _writer: writer,
            staged: BTreeMap::new(),
        }))
    }
}
