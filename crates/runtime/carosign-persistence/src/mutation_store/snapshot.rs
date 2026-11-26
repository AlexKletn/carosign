use crate::model::StorageError;
use crate::store::{ReadTransaction, Table};

use super::read_view::ReadView;
use super::schema::{SCHEMA_V4, corrupt};

/// Checks prepared, armed and observed changes in one image.
/// An armed or observed alteration requires verification and does not permit re-sending.
pub fn scan_prepared_v4_snapshot(
    tx: &dyn ReadTransaction,
) -> Result<crate::recovery::StartupScopeScan, StorageError> {
    let view = ReadView { inner: tx };
    scan_prepared_view(&view)
}

/// It checks for the exact prepared mutation and not sending it in a single image, and it
/// doesn't confirm resource harvesting, and it doesn't allow recording.
pub fn inspect_prepared_v4(
    store: &dyn crate::store::MetadataStore,
    mutation: carosign_core::ids::MutationId,
    operation: carosign_core::ids::OperationId,
) -> Result<crate::recovery::MutationSnapshot, StorageError> {
    let tx = store.begin_read()?;
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    let view = ReadView { inner: &*tx };
    let scan = scan_prepared_view(&view)?;
    if !scan.scan.safe_to_close.contains(&mutation) {
        return Err(StorageError::GenerationConflict);
    }
    let snapshot =
        crate::recovery::read_mutation_snapshot_v3_in_snapshot(&view, mutation, operation)?;
    let record = &snapshot.mutation;
    if record.phase != crate::model::MutationPhase::Prepared
        || record.worker.is_some()
        || record.command.is_some()
        || record.epoch.is_some()
        || record.recovery_ref.is_some()
        || !snapshot.outbox.is_empty()
    {
        return Err(StorageError::GenerationConflict);
    }
    Ok(snapshot)
}

fn scan_prepared_view(
    view: &dyn ReadTransaction,
) -> Result<crate::recovery::StartupScopeScan, StorageError> {
    for (_, value) in view.scan_prefix(Table::Enrollments, &[])? {
        let entry = crate::enrollment_codec::decode_v3(&value).map_err(|_| corrupt())?;
        // The contract and secure storage verification are not yet connected.
        if entry.binding_kind != crate::mutation_plan::StoredBindingKind::ProviderOpaque {
            return Err(corrupt());
        }
    }
    let scan = crate::recovery::StartupScan::scan_v3_snapshot(view)?;
    // The unfinished stages keep the area locked, the permanently closed records are checked
    // by a common scanner and do not prevent a new launch.
    for mutation in &scan.scan.needs_decision {
        let value = view
            .get(Table::Mutations, mutation.as_bytes())?
            .ok_or_else(corrupt)?;
        if crate::mutation_records::decode_mutation(&value)?.phase
            != crate::model::MutationPhase::NoChange
        {
            return Err(corrupt());
        }
    }
    for (key, value) in view.scan_prefix(Table::EffectOutbox, &[])? {
        if crate::mutation_records::decode_outbox(&key, &value)?
            .mutation
            .is_none()
        {
            return Err(corrupt());
        }
    }
    for mutation in &scan.scan.needing_reconciliation {
        let value = view
            .get(Table::Mutations, mutation.as_bytes())?
            .ok_or_else(corrupt)?;
        if !matches!(
            crate::mutation_records::decode_mutation(&value)?.phase,
            crate::model::MutationPhase::DispatchArmed
                | crate::model::MutationPhase::ProviderObserved
                | crate::model::MutationPhase::RegistryCommitted
                | crate::model::MutationPhase::NeedsReconciliation
        ) {
            return Err(corrupt());
        }
    }
    Ok(scan)
}
