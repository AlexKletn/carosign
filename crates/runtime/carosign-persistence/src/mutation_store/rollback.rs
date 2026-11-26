use crate::store::{DurableReceipt, Table};

use super::DurableRegistryFailure;
use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

pub fn commit_enrollment_rollback_v4(
    store: &dyn crate::store::MetadataStore,
    input: &crate::mutation_records::ObservedMutation,
) -> Result<DurableReceipt, DurableRegistryFailure> {
    let mut tx = store
        .begin_write()
        .map_err(DurableRegistryFailure::BeforeCommit)?;
    let meta = tx
        .scan_prefix(Table::Meta, &[])
        .map_err(DurableRegistryFailure::BeforeCommit)?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(DurableRegistryFailure::BeforeCommit(corrupt()));
    }
    crate::mutation_records::stage_enrollment_rollback(&mut RecordView { inner: &mut *tx }, input)
        .map_err(DurableRegistryFailure::BeforeCommit)?;
    tx.commit_durable()
        .map_err(DurableRegistryFailure::CommitUnknown)
}
