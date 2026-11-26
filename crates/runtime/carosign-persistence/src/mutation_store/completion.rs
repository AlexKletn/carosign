use crate::model::StorageError;
use crate::store::{DurableReceipt, ReadTransaction, Table, WriteTransaction};

use super::DurableRegistryFailure;
use super::read_view::ReadView;
use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Prepares the completion and release of the v4 circuit area after cleaning.
pub fn stage_completed_v4(
    tx: &mut dyn WriteTransaction,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<(), StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    crate::mutation_records::stage_completed(&mut RecordView { inner: tx }, mutation, operation)
}

/// Completes the change after confirmed by the inducing cleaning.
pub fn complete_v4(
    store: &dyn crate::store::MetadataStore,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<DurableReceipt, DurableRegistryFailure> {
    let mut tx = store
        .begin_write()
        .map_err(DurableRegistryFailure::BeforeCommit)?;
    stage_completed_v4(&mut *tx, mutation, operation)
        .map_err(DurableRegistryFailure::BeforeCommit)?;
    tx.commit_durable()
        .map_err(DurableRegistryFailure::CommitUnknown)
}

/// Reads a stable completed result from one image of the v4 scheme; additionally checks the
/// ownership of the operation, the log, the queue and the lock area.
pub fn read_completed_v4(
    store: &dyn crate::store::MetadataStore,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<crate::model::MutationRecord, StorageError> {
    let tx = store.begin_read()?;
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    let view = ReadView { inner: &*tx };
    let bytes = view
        .get(Table::Mutations, mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let record = crate::mutation_records::decode_mutation(&bytes)?;
    if record.mutation != mutation || record.operation != operation {
        return Err(corrupt());
    }
    crate::recovery::completed_outcome_in_snapshot(&view, &record)?;
    Ok(record)
}
