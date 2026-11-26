use super::{
    MetadataStore, MutationId, MutationPhase, MutationSnapshot, OperationId, StorageError,
    check_idempotency_for_mutation,
};

/// Reads the unfinished mutation, its blockage, queue and area in one picture.
///
/// The caller must first confirm the base diagram and authority to the area, the registry and
/// the index are checked in both directions, this does not prove the physical condition of
/// the device, if the error does not change the base.
pub fn read_mutation_snapshot_v3(
    store: &dyn MetadataStore,
    mutation: MutationId,
    operation: OperationId,
) -> Result<MutationSnapshot, StorageError> {
    let tx = store.begin_read()?;
    read_mutation_snapshot_v3_in_snapshot(&*tx, mutation, operation)
}

pub(crate) fn read_mutation_snapshot_v3_in_snapshot(
    tx: &dyn crate::store::ReadTransaction,
    mutation: MutationId,
    operation: OperationId,
) -> Result<MutationSnapshot, StorageError> {
    use crate::store::Table;
    let corrupt = || StorageError::Corrupt {
        what: "несогласованный снимок восстановления v3",
    };
    let bytes = tx
        .get(Table::Mutations, mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let record = crate::mutation_records::decode_mutation(&bytes)?;
    if record.mutation != mutation || record.operation != operation {
        return Err(corrupt());
    }
    if record.phase == MutationPhase::Completed {
        return Err(StorageError::GenerationConflict);
    }
    check_idempotency_for_mutation(tx, &record)?;
    // A separate recovery may not miss a second journal with the same OperationId Or another
    // incomplete mutation of the same region.
    for (key, bytes) in tx.scan_prefix(Table::Mutations, &[])? {
        let other = crate::mutation_records::decode_mutation(&bytes)?;
        if key.as_slice() != other.mutation.as_bytes()
            || other.mutation != mutation
                && (other.operation == operation
                    || other.phase != MutationPhase::Completed
                        && other.scope_key() == record.scope_key())
        {
            return Err(corrupt());
        }
    }
    if tx
        .get(Table::MutationScopeLock, &record.scope_key())?
        .as_deref()
        != Some(mutation.as_bytes().as_slice())
    {
        return Err(corrupt());
    }
    let mut outbox = Vec::new();
    for (key, bytes) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let effect = crate::mutation_records::decode_outbox(&key, &bytes)?;
        let same_mutation = effect.mutation == Some(mutation);
        let same_command = record.command.is_some() && effect.command == record.command;
        if same_mutation || same_command {
            if effect.mutation != Some(mutation) {
                return Err(corrupt());
            }
            crate::mutation_records::validate_mutation_outbox(&record, &effect)?;
            if !outbox.is_empty() {
                return Err(corrupt());
            }
            outbox.push(effect);
        } else if effect.key.operation == operation && effect.mutation.is_some() {
            return Err(corrupt());
        }
    }
    if (matches!(
        record.phase,
        MutationPhase::DispatchArmed
            | MutationPhase::ProviderObserved
            | MutationPhase::RegistryCommitted
            | MutationPhase::NeedsReconciliation
    ) || record.phase == MutationPhase::NoChange && record.command.is_some())
        && outbox.is_empty()
    {
        return Err(corrupt());
    }
    let mut enrollments = Vec::new();
    for (key, id) in tx.scan_prefix(Table::EnrollmentIndex, &record.scope_key())? {
        if key.len() != crate::limits::ENROLLMENT_INDEX_KEY_LEN
            || key[crate::limits::SCOPE_LOCK_KEY_LEN..] != id[..]
        {
            return Err(corrupt());
        }
        let bytes = tx.get(Table::Enrollments, &id)?.ok_or_else(corrupt)?;
        let entry = crate::enrollment_codec::decode_v3(&bytes).map_err(|_| corrupt())?;
        if crate::redb::codec::enrollment_index_key(&entry).as_slice() != key.as_slice() {
            return Err(corrupt());
        }
        enrollments.push(entry);
    }
    for (key, bytes) in tx.scan_prefix(Table::Enrollments, &[])? {
        let entry = crate::enrollment_codec::decode_v3(&bytes).map_err(|_| corrupt())?;
        if key.as_slice() != entry.enrollment.id.as_bytes() {
            return Err(corrupt());
        }
        if entry.subject == record.subject
            && entry.enrollment.authenticator == record.authenticator
            && tx
                .get(
                    Table::EnrollmentIndex,
                    &crate::redb::codec::enrollment_index_key(&entry),
                )?
                .as_deref()
                != Some(entry.enrollment.id.as_bytes().as_slice())
        {
            return Err(corrupt());
        }
    }
    Ok(MutationSnapshot {
        mutation: record,
        outbox,
        enrollments,
    })
}
