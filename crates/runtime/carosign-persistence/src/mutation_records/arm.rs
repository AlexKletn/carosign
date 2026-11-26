use super::codec::encode_record_for_schema;
use super::ownership::{check_idempotency_owner, check_journal_owner};
use super::record_error::corrupt;
use super::{decode_mutation, decode_outbox, encode_outbox, validate_mutation_outbox};
use crate::journal::ArmMutation;
use crate::model::{MutationPhase, StorageError};
use crate::mutation_plan::StoredBindingKind;
use crate::outbox::{EffectOutboxRecord, EffectPhase};
use crate::store::{Table, WriteTransaction};

/// Prepare. `DispatchArmed` and a queue in a single transaction v3.
///
/// The address must correspond to the effect of changing the send from the solution of the
/// engine; the worker's binding and authority checks the trusted calling code.
/// In case of any error, the transaction must be interrupted, the function does not confirm
/// stability and does not issue a ticket for sending.
pub fn stage_arm(
    tx: &mut dyn WriteTransaction,
    input: &ArmMutation,
    effect: carosign_engine::PlannedEffect,
) -> Result<(), StorageError> {
    use carosign_engine::EffectSpec;
    use carosign_engine::transition::{EffectKey, EffectPosition};

    if effect.spec != EffectSpec::SendMutatingCommand || effect.transition.value() == 0 {
        return Err(corrupt());
    }
    let bytes = tx
        .get(Table::Mutations, input.mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let mut record = decode_mutation(&bytes)?;
    if record.mutation != input.mutation {
        return Err(corrupt());
    }
    if record.phase != MutationPhase::Prepared {
        return Err(StorageError::GenerationConflict);
    }
    if record.command.is_some_and(|value| value != input.command)
        || record.worker.is_some_and(|value| value != input.worker)
        || record.epoch.is_some_and(|value| value != input.epoch)
    {
        return Err(StorageError::GenerationConflict);
    }
    if tx
        .get(Table::MutationScopeLock, &record.scope_key())?
        .as_deref()
        != Some(input.mutation.as_bytes().as_slice())
    {
        return Err(corrupt());
    }
    check_journal_owner(tx, &record)?;
    check_idempotency_owner(tx, &record)?;
    crate::journal_store::check_current_scope_v3(tx, &record)?;
    if record.plan.as_ref().is_some_and(|plan| {
        plan.old_target.is_some_and(|old| {
            plan.pre_bindings.iter().any(|binding| {
                binding.enrollment == old && binding.kind == StoredBindingKind::ScopedVault
            })
        })
    }) {
        return Err(StorageError::Unavailable);
    }
    let outbox = EffectOutboxRecord {
        key: EffectKey {
            operation: record.operation,
            transition_index: effect.transition,
            position: EffectPosition::new(effect.position),
        },
        command: Some(input.command),
        phase: EffectPhase::Armed,
        mutation: Some(input.mutation),
    };
    let outbox_key = outbox.storage_key();
    if tx.get(Table::EffectOutbox, &outbox_key)?.is_some() {
        return Err(StorageError::GenerationConflict);
    }
    for (key, value) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let existing = decode_outbox(&key, &value)?;
        if existing.mutation == Some(input.mutation) || existing.command == Some(input.command) {
            return Err(StorageError::GenerationConflict);
        }
    }
    record.phase = MutationPhase::DispatchArmed;
    record.worker = Some(input.worker);
    record.epoch = Some(input.epoch);
    record.command = Some(input.command);
    record.recovery_ref = input.recovery_ref.clone();
    validate_mutation_outbox(&record, &outbox)?;
    let record_bytes = encode_record_for_schema(tx, &record)?;
    let outbox_bytes = encode_outbox(&outbox)?;
    tx.put(Table::Mutations, input.mutation.as_bytes(), &record_bytes)?;
    tx.put(Table::EffectOutbox, &outbox_key, &outbox_bytes)?;
    Ok(())
}
