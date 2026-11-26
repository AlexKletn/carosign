use super::ownership::{check_idempotency_owner, check_journal_owner};
use super::record_error::corrupt;
use super::{decode_mutation, decode_outbox, encode_mutation};
use crate::model::{MutationPhase, StorageError};
use crate::store::{Table, WriteTransaction};
use carosign_core::ids::{MutationId, OperationId};

/// Preparing metadata `NoChange` for a change that was not authorized to be sent.
///
/// Stable locking is maintained until confirmed cleaning and completion.
/// The caller is obliged to interrupt the transaction in case of an error; the success of
/// this feature is not an evidence of the recording or release of resources.
pub fn stage_pre_dispatch_no_change(
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
    if record.phase != MutationPhase::Prepared
        || record.worker.is_some()
        || record.epoch.is_some()
        || record.command.is_some()
        || record.recovery_ref.is_some()
    {
        return Err(StorageError::GenerationConflict);
    }
    if tx
        .get(Table::MutationScopeLock, &record.scope_key())?
        .as_deref()
        != Some(mutation.as_bytes().as_slice())
    {
        return Err(corrupt());
    }
    check_journal_owner(tx, &record)?;
    check_idempotency_owner(tx, &record)?;
    for (key, bytes) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let effect = decode_outbox(&key, &bytes)?;
        if effect.mutation == Some(mutation)
            || effect.key.operation == operation && effect.command.is_some()
        {
            return Err(corrupt());
        }
    }
    crate::journal_store::check_current_scope_v3(tx, &record)?;
    record.phase = MutationPhase::NoChange;
    tx.put(
        Table::Mutations,
        mutation.as_bytes(),
        &encode_mutation(&record)?,
    )?;
    Ok(())
}
