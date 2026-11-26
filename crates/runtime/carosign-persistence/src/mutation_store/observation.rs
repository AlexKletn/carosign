use crate::model::StorageError;
use crate::store::{Table, WriteTransaction};

use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Prepares owner-confirmed surveillance and queue updates in one v4 transaction. This does
/// not prove the outcome of the change in the provider.
/// In case of an error, the caller is obliged to interrupt the transaction; after an
/// indefinite commit, the area remains closed until reconciliation.
pub fn stage_observation_v4(
    tx: &mut dyn WriteTransaction,
    input: &crate::mutation_records::ObservedMutation,
) -> Result<(), StorageError> {
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    crate::mutation_records::stage_observation(&mut RecordView { inner: tx }, input)
}
