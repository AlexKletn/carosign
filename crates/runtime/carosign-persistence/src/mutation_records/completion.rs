use super::ownership::{check_idempotency_owner, check_journal_owner};
use super::record_error::corrupt;
use super::{decode_mutation, encode_mutation};
use crate::model::{MutationPhase, MutationTerminalOutcome, StorageError};
use crate::store::{Table, WriteTransaction};
use carosign_core::ids::{MutationId, OperationId};

/// Completes only an already applied change or a recorded NoChange.
/// The caller is obliged to confirm the termination of the service provider and cleaning
/// before the call; if the refusal or unknown fixation, the area remains closed.
pub fn stage_completed(
    tx: &mut dyn WriteTransaction,
    mutation: MutationId,
    operation: OperationId,
) -> Result<(), StorageError> {
    let bytes = tx
        .get(Table::Mutations, mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let mut record = decode_mutation(&bytes)?;
    if record.mutation != mutation || record.operation != operation {
        return Err(corrupt());
    }
    let outcome = match record.phase {
        MutationPhase::RegistryCommitted => MutationTerminalOutcome::Committed,
        MutationPhase::NoChange => MutationTerminalOutcome::NoChange,
        _ => return Err(StorageError::GenerationConflict),
    };
    if tx
        .get(Table::MutationScopeLock, &record.scope_key())?
        .as_deref()
        != Some(mutation.as_bytes().as_slice())
    {
        return Err(corrupt());
    }
    check_journal_owner(tx, &record)?;
    check_idempotency_owner(tx, &record)?;
    record.phase = MutationPhase::Completed;
    record.terminal_outcome = Some(outcome);
    tx.put(
        Table::Mutations,
        mutation.as_bytes(),
        &encode_mutation(&record)?,
    )?;
    tx.delete(Table::MutationScopeLock, &record.scope_key())?;
    Ok(())
}
