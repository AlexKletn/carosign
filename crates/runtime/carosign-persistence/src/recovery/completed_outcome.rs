use super::{
    MetadataStore, MutationId, MutationPhase, MutationRecord, OperationId, StorageError,
    check_idempotency_for_mutation,
};

/// Reads the stable completed outcome; the right of observation checks the caller.
///
/// Another incomplete mutation may already own the same area; its blocking and recording are
/// checked in the same image.
pub fn read_completed_mutation_v3(
    store: &dyn MetadataStore,
    mutation: MutationId,
    operation: OperationId,
) -> Result<crate::model::MutationTerminalOutcome, StorageError> {
    use crate::store::Table;
    let corrupt = || StorageError::Corrupt {
        what: "неверный завершённый исход изменения",
    };
    let tx = store.begin_read()?;
    let bytes = tx
        .get(Table::Mutations, mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let record = crate::mutation_records::decode_mutation(&bytes)?;
    if record.mutation != mutation || record.operation != operation {
        return Err(corrupt());
    }
    completed_outcome_in_snapshot(&*tx, &record)
}

/// Looks for a complete mutation from the operation in a single v3 image.
///
/// The caller is obliged to separately verify the right of surveillance, the unknown
/// operation returns `None`Repeated operation in the journal is considered spoilage.
pub fn read_completed_by_operation_v3(
    store: &dyn MetadataStore,
    operation: OperationId,
) -> Result<Option<crate::model::MutationTerminalOutcome>, StorageError> {
    use crate::store::Table;
    let corrupt = || StorageError::Corrupt {
        what: "неверный журнал операций",
    };
    let tx = store.begin_read()?;
    let mut found = None;
    for (key, bytes) in tx.scan_prefix(Table::Mutations, &[])? {
        let record = crate::mutation_records::decode_mutation(&bytes)?;
        if key.as_slice() != record.mutation.as_bytes() {
            return Err(corrupt());
        }
        if record.operation == operation {
            if found.is_some() {
                return Err(corrupt());
            }
            found = Some(record);
        }
    }
    found
        .map(|record| completed_outcome_in_snapshot(&*tx, &record))
        .transpose()
}

pub(crate) fn completed_outcome_in_snapshot(
    tx: &dyn crate::store::ReadTransaction,
    record: &MutationRecord,
) -> Result<crate::model::MutationTerminalOutcome, StorageError> {
    use crate::store::Table;
    let corrupt = || StorageError::Corrupt {
        what: "неверный завершённый исход изменения",
    };
    if record.phase != MutationPhase::Completed {
        return Err(StorageError::GenerationConflict);
    }
    check_idempotency_for_mutation(tx, record)?;
    // Separate reading on MutationId must not bypass the same uniqueness check as the search
    // for OperationId.
    for (key, bytes) in tx.scan_prefix(Table::Mutations, &[])? {
        let other = crate::mutation_records::decode_mutation(&bytes)?;
        if key.as_slice() != other.mutation.as_bytes()
            || other.operation == record.operation && other.mutation != record.mutation
        {
            return Err(corrupt());
        }
    }
    // The queue may already be removed, but the remaining entries must be consistent with the
    // journal and not repeat the changing command.
    let mut found_outbox = false;
    for (key, bytes) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let effect = crate::mutation_records::decode_outbox(&key, &bytes)?;
        let same_mutation = effect.mutation == Some(record.mutation);
        let same_command = record.command.is_some() && effect.command == record.command;
        if same_mutation || same_command {
            if found_outbox || effect.mutation != Some(record.mutation) {
                return Err(corrupt());
            }
            crate::mutation_records::validate_mutation_outbox(record, &effect)?;
            found_outbox = true;
        } else if effect.key.operation == record.operation && effect.mutation.is_some() {
            return Err(corrupt());
        }
    }
    if let Some(lock) = tx.get(Table::MutationScopeLock, &record.scope_key())? {
        let owner = MutationId::try_from(lock.as_slice()).map_err(|_| corrupt())?;
        if owner == record.mutation {
            return Err(corrupt());
        }
        let bytes = tx
            .get(Table::Mutations, owner.as_bytes())?
            .ok_or_else(corrupt)?;
        let other = crate::mutation_records::decode_mutation(&bytes)?;
        if other.mutation != owner
            || other.phase == MutationPhase::Completed
            || other.scope_key() != record.scope_key()
            || other.operation == record.operation
        {
            return Err(corrupt());
        }
    }
    record.terminal_outcome.ok_or_else(corrupt)
}
