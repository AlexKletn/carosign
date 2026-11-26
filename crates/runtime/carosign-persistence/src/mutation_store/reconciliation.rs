use crate::model::StorageError;
use crate::store::{Table, WriteTransaction};

use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Atomically prepares the closure of the area with an unknown outcome.
/// Sustainable fixation and verification of powers remain with the caller.
pub fn stage_needs_reconciliation_v4(
    tx: &mut dyn WriteTransaction,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<(), StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    crate::mutation_records::stage_needs_reconciliation(
        &mut RecordView { inner: tx },
        mutation,
        operation,
    )
}
