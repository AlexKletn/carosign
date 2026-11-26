use carosign_core::ids::OperationId;

use crate::mutation_records::decode_mutation;
use crate::store::{MetadataStore, ReadTransaction, Table};

use super::read_view::ReadView;
use super::schema::corrupt;
use super::{
    DurableRegistryFailure, scan_prepared_v4_snapshot, stage_completed_v4,
    stage_pre_dispatch_no_change_v4,
};

pub fn recover_unarmed_v4(
    store: &dyn MetadataStore,
    cleanup_confirmed: impl Fn(OperationId) -> bool,
) -> Result<usize, DurableRegistryFailure> {
    let before_commit = DurableRegistryFailure::BeforeCommit;
    let tx = store.begin_read().map_err(before_commit)?;
    let scan = scan_prepared_v4_snapshot(&*tx).map_err(before_commit)?;
    let view = ReadView { inner: &*tx };
    let mut recoverable = Vec::new();
    for mutation in scan.scan.safe_to_close {
        let value = view
            .get(Table::Mutations, mutation.as_bytes())
            .map_err(before_commit)?
            .ok_or_else(|| before_commit(corrupt()))?;
        let record = decode_mutation(&value).map_err(before_commit)?;
        if cleanup_confirmed(record.operation) {
            recoverable.push((mutation, record.operation));
        }
    }
    let remaining = scan
        .blocked
        .len()
        .checked_sub(recoverable.len())
        .ok_or_else(|| before_commit(corrupt()))?;
    drop(tx);

    for (mutation, operation) in recoverable {
        let mut tx = store.begin_write().map_err(before_commit)?;
        let staged = stage_pre_dispatch_no_change_v4(&mut *tx, mutation, operation)
            .and_then(|()| stage_completed_v4(&mut *tx, mutation, operation));
        if let Err(error) = staged {
            tx.abort();
            return Err(before_commit(error));
        }
        tx.commit_durable()
            .map_err(DurableRegistryFailure::CommitUnknown)?;
    }
    Ok(remaining)
}
