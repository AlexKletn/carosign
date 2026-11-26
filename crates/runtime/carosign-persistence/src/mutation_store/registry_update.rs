use crate::model::StorageError;
use crate::store::{DurableReceipt, Table, WriteTransaction};

use super::DurableRegistryFailure;
use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Prepares the atomic application of the result to the registry of the v4 scheme.
/// Confirmation of origin and fixation of the transaction lie on the caller.
pub fn stage_registry_update_v4(
    tx: &mut dyn WriteTransaction,
    input: &crate::mutation_records::RegistryUpdate<'_>,
) -> Result<(), StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    crate::mutation_records::stage_registry_update(&mut RecordView { inner: tx }, input)
}

/// Applies a proven result and confirms a stable fixation.
pub fn commit_registry_v4(
    store: &dyn crate::store::MetadataStore,
    input: &crate::mutation_records::RegistryUpdate<'_>,
) -> Result<DurableReceipt, DurableRegistryFailure> {
    let mut tx = store
        .begin_write()
        .map_err(DurableRegistryFailure::BeforeCommit)?;
    stage_registry_update_v4(&mut *tx, input).map_err(DurableRegistryFailure::BeforeCommit)?;
    tx.commit_durable()
        .map_err(DurableRegistryFailure::CommitUnknown)
}
