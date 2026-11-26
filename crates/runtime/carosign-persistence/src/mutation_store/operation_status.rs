use carosign_core::ids::OperationId;

use crate::model::{MutationPhase, MutationRecord, StorageError};
use crate::mutation_records::decode_mutation;
use crate::store::{MetadataStore, ReadTransaction, Table};

use super::{
    read_view::ReadView,
    schema::{SCHEMA_V4, corrupt},
};

pub fn read_mutation_by_operation_v4(
    store: &dyn MetadataStore,
    operation: OperationId,
) -> Result<Option<MutationRecord>, StorageError> {
    let tx = store.begin_read()?;
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    let view = ReadView { inner: &*tx };
    let mut result = None;
    for (key, bytes) in view.scan_prefix(Table::Mutations, &[])? {
        let record = decode_mutation(&bytes)?;
        if key != record.mutation.as_bytes().as_slice() {
            return Err(corrupt());
        }
        if record.operation == operation {
            if result.is_some() {
                return Err(corrupt());
            }
            if record.phase == MutationPhase::Completed {
                crate::recovery::completed_outcome_in_snapshot(&view, &record)?;
            }
            result = Some(record);
        }
    }
    Ok(result)
}
