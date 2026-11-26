use crate::model::StorageError;
use crate::store::Table;

use super::read_view::ReadView;
use super::schema::{SCHEMA_V4, corrupt};
use super::snapshot::scan_prepared_v4_snapshot;

/// Reads the change plan for independent reconciliation from one v4 image.
///
/// The caller confirms the origin of the expected area and era, the plan does not confirm the
/// state of the device and does not allow re-sending.
pub fn read_reconciliation_plan_v4(
    store: &dyn crate::store::MetadataStore,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
    subject: carosign_core::identity::SubjectRef,
    authenticator: carosign_core::ids::AuthenticatorId,
    epoch: carosign_core::device::DeviceEpoch,
) -> Result<carosign_core::operation::plan::MutationPlan, StorageError> {
    let tx = store.begin_read()?;
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    let scan = scan_prepared_v4_snapshot(&*tx)?;
    if !scan.scan.needing_reconciliation.contains(&mutation) {
        return Err(StorageError::GenerationConflict);
    }
    let view = ReadView { inner: &*tx };
    let record =
        crate::recovery::read_mutation_snapshot_v3_in_snapshot(&view, mutation, operation)?
            .mutation;
    if record.subject != subject
        || record.authenticator != authenticator
        || record.epoch != Some(epoch)
    {
        return Err(corrupt());
    }
    record
        .plan
        .ok_or_else(corrupt)?
        .to_core(crate::mutation_plan::Scope {
            subject,
            authenticator,
        })
        .map_err(|_| corrupt())
}
