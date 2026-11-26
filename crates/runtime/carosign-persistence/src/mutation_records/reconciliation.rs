use super::codec::encode_record_for_schema;
use super::ownership::check_journal_owner;
use super::record_error::corrupt;
use super::{decode_mutation, decode_outbox, encode_outbox, validate_mutation_outbox};
use crate::model::{MutationPhase, StorageError};
use crate::outbox::EffectPhase;
use crate::store::{Table, WriteTransaction};
use carosign_core::ids::{MutationId, OperationId};

/// Prepares a mark of uncertainty and queue in a single transaction.
///
/// `RegistryCommitted` and `NoChange` They can be the result of a successful record with a
/// lost confirmation: they are not allowed to be overwritten. The result does not confirm
/// stability; the fixation remains with the caller. The service lineup and the circuit
/// validation are not yet connected to this internal step.
pub(crate) fn stage_needs_reconciliation(
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
    if !matches!(
        record.phase,
        MutationPhase::DispatchArmed
            | MutationPhase::ProviderObserved
            | MutationPhase::NeedsReconciliation
    ) {
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
    let mut linked = None;
    for (key, bytes) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let effect = decode_outbox(&key, &bytes)?;
        let same_mutation = effect.mutation == Some(mutation);
        let same_command = record.command.is_some() && effect.command == record.command;
        if same_mutation || same_command {
            if linked.is_some() || effect.mutation != Some(mutation) {
                return Err(corrupt());
            }
            validate_mutation_outbox(&record, &effect)?;
            linked = Some((key, effect));
        } else if effect.key.operation == operation && effect.mutation.is_some() {
            return Err(corrupt());
        }
    }
    let (key, mut effect) = linked.ok_or_else(corrupt)?;
    record.phase = MutationPhase::NeedsReconciliation;
    effect.phase = EffectPhase::ReconciliationRequired;
    let record_bytes = encode_record_for_schema(tx, &record)?;
    let effect_bytes = encode_outbox(&effect)?;
    tx.put(Table::Mutations, mutation.as_bytes(), &record_bytes)?;
    tx.put(Table::EffectOutbox, &key, &effect_bytes)?;
    Ok(())
}

/// Fixes the uncertainty mark and returns the vault confirmation.
///
/// Before calling, the staff of the service is obliged to check the scheme and status of
/// migration.
/// A fixation error can mean a lost confirmation: resending a command is prohibited, and
/// further resolution requires reading the actual phase.
#[allow(dead_code)]
pub(crate) fn persist_needs_reconciliation(
    store: &dyn crate::store::MetadataStore,
    mutation: MutationId,
    operation: OperationId,
) -> Result<crate::store::DurableReceipt, StorageError> {
    let mut tx = store.begin_write()?;
    stage_needs_reconciliation(&mut *tx, mutation, operation)?;
    tx.commit_durable()
}
