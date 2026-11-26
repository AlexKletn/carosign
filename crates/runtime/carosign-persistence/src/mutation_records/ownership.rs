use super::record_error::corrupt;
use super::{decode_idempotency, decode_mutation};
use crate::model::{MutationPhase, MutationRecord, StorageError};
use crate::store::{Table, WriteTransaction};

// Checks the back of the lock: another incomplete log of the same area or a repeat of the
// operation number indicates a damaged base.
pub(super) fn check_journal_owner(
    tx: &dyn WriteTransaction,
    record: &MutationRecord,
) -> Result<(), StorageError> {
    for (key, bytes) in tx.scan_prefix(Table::Mutations, &[])? {
        let other = decode_mutation(&bytes)?;
        if key.as_slice() != other.mutation.as_bytes()
            || other.mutation != record.mutation
                && (other.operation == record.operation
                    || other.phase != MutationPhase::Completed
                        && other.scope_key() == record.scope_key())
        {
            return Err(corrupt());
        }
    }
    Ok(())
}

pub(super) fn check_idempotency_owner(
    tx: &dyn WriteTransaction,
    record: &MutationRecord,
) -> Result<(), StorageError> {
    let mut found = false;
    for (key, bytes) in tx.scan_prefix(Table::Idempotency, &[])? {
        let entry = decode_idempotency(&key, &bytes)?;
        if (entry.operation == record.operation && entry.mutation != Some(record.mutation))
            || (entry.mutation == Some(record.mutation) && entry.operation != record.operation)
        {
            return Err(corrupt());
        }
        found |= entry.operation == record.operation && entry.mutation == Some(record.mutation);
    }
    if !found {
        return Err(corrupt());
    }
    Ok(())
}
