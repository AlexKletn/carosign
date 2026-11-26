use crate::model::StorageError;
use crate::store::{Table, WriteTransaction};

use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Builds a verified projection of the prepared change in the v4 scheme.
///
/// The caller saves the transaction and is obliged to separately verify the authority,
/// worker and era.Projection does not allow sending to the provider.
pub fn enrollment_projection_v4(
    tx: &mut dyn WriteTransaction,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<crate::projection::Projection, StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    crate::mutation_records::enrollment_projection(&RecordView { inner: tx }, mutation, operation)
}
