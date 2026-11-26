use crate::model::StorageError;
use crate::store::{Table, WriteTransaction};

use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Prepares the refusal before sending, keeping the lock until confirmed cleaning.
/// Success does not mean a stable fixation; if a transaction is mistaken, it should be
/// interrupted.
pub fn stage_pre_dispatch_no_change_v4(
    tx: &mut dyn WriteTransaction,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<(), StorageError> {
    stage_pre_dispatch_no_change_with_schema(tx, mutation, operation, SCHEMA_V4)
}

/// Prepares a refusal before sending a verified v5 owner into a transaction.
/// Confirmation of cleaning and stable fixation provides a caller.
#[allow(dead_code)]
pub(crate) fn stage_pre_dispatch_no_change_v5(
    tx: &mut crate::redb::backend::GuardedWriteTx<'_>,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<(), StorageError> {
    stage_pre_dispatch_no_change_with_schema(tx, mutation, operation, b"BSC1\0\0\0\x05\0\0\0\0")
}

fn stage_pre_dispatch_no_change_with_schema(
    tx: &mut dyn WriteTransaction,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
    schema: &[u8],
) -> Result<(), StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != schema {
        return Err(corrupt());
    }
    crate::mutation_records::stage_pre_dispatch_no_change(
        &mut RecordView { inner: tx },
        mutation,
        operation,
    )
}
