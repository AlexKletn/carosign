use super::{
    DurableReceipt, MetadataStore, ReadTransaction, StorageError, Table, TableMap, WriteTransaction,
};
use crate::redb::codec::{Envelope, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Records the value in memory with the shell of the circuit - as in battle.
pub fn put_enveloped(
    store: &MemoryStore,
    table: Table,
    key: &[u8],
    payload: &[u8],
) -> Result<(), StorageError> {
    let mut tx = store.begin_write()?;
    let bytes = Envelope {
        schema: SCHEMA_VERSION,
        payload: payload.to_vec(),
    }
    .encode()?;
    tx.put(table, key, &bytes)?;
    tx.commit_durable()?;
    Ok(())
}

mod memory_store;
pub use memory_store::MemoryStore;
mod memory_read;
pub use memory_read::MemoryRead;
mod memory_write;
pub use memory_write::MemoryWrite;
