use super::record_error::corrupt;
use super::{
    decode_idempotency, decode_mutation, decode_outbox, encode_mutation, validate_mutation_outbox,
};
use crate::idempotency::{Reservation, ReservationError};
use crate::journal::PrepareMutation;
use crate::model::{IdempotencyRecord, MutationPhase, MutationRecord, StorageError};
use crate::store::{Table, WriteTransaction};
use carosign_core::ids::MutationId;

/// Prepares a log, lock and idempotence in a single v3 transaction.
///
/// The caller must first confirm the database schema and trusted origin of the normalized
/// digest. `Staged` It does not indicate a stable record. `Existing` It does not give the
/// right to observe an old operation without re-authorization.
pub fn stage_prepared(
    tx: &mut dyn WriteTransaction,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
) -> Result<Reservation, ReservationError> {
    stage_prepared_for_schema(tx, input, idempotency)
}

fn stage_prepared_for_schema(
    tx: &mut dyn WriteTransaction,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
) -> Result<Reservation, ReservationError> {
    crate::journal::validate_prepare(input).map_err(ReservationError::Storage)?;
    if idempotency.operation != input.operation || idempotency.mutation != Some(input.mutation) {
        return Err(ReservationError::Storage(corrupt()));
    }
    if let Some(existing) = crate::idempotency::lookup(tx, idempotency)? {
        let existing_mutation = existing
            .mutation
            .ok_or_else(|| ReservationError::Storage(corrupt()))?;
        let bytes = tx
            .get(Table::Mutations, existing_mutation.as_bytes())
            .map_err(ReservationError::Storage)?
            .ok_or_else(|| ReservationError::Storage(corrupt()))?;
        let record = decode_mutation(&bytes).map_err(ReservationError::Storage)?;
        if record.mutation != existing_mutation
            || record.operation != existing.operation
            || record.subject != input.subject
            || record.authenticator != input.authenticator
            || record.expected_generation != input.expected_generation
            || record.plan.as_ref().is_none_or(|plan| {
                plan.transition != input.plan.transition || plan.old_target != input.plan.old_target
            })
        {
            return Err(ReservationError::Storage(corrupt()));
        }
        let lock = tx
            .get(Table::MutationScopeLock, &record.scope_key())
            .map_err(ReservationError::Storage)?;
        if record.phase != MutationPhase::Completed {
            if lock.as_deref() != Some(existing_mutation.as_bytes().as_slice()) {
                return Err(ReservationError::Storage(corrupt()));
            }
        } else if let Some(owner) = lock {
            let owner = MutationId::try_from(owner.as_slice())
                .map_err(|_| ReservationError::Storage(corrupt()))?;
            let bytes = tx
                .get(Table::Mutations, owner.as_bytes())
                .map_err(ReservationError::Storage)?
                .ok_or_else(|| ReservationError::Storage(corrupt()))?;
            let other = decode_mutation(&bytes).map_err(ReservationError::Storage)?;
            if other.mutation != owner
                || other.mutation == record.mutation
                || other.phase == MutationPhase::Completed
                || other.scope_key() != record.scope_key()
                || other.operation == record.operation
            {
                return Err(ReservationError::Storage(corrupt()));
            }
        }
        for (key, value) in tx
            .scan_prefix(Table::Mutations, &[])
            .map_err(ReservationError::Storage)?
        {
            let other = decode_mutation(&value).map_err(ReservationError::Storage)?;
            if key.as_slice() != other.mutation.as_bytes()
                || other.mutation != record.mutation
                    && (other.operation == record.operation
                        || record.phase != MutationPhase::Completed
                            && other.phase != MutationPhase::Completed
                            && other.scope_key() == record.scope_key())
            {
                return Err(ReservationError::Storage(corrupt()));
            }
        }
        for (key, value) in tx
            .scan_prefix(Table::Idempotency, &[])
            .map_err(ReservationError::Storage)?
        {
            let other = decode_idempotency(&key, &value).map_err(ReservationError::Storage)?;
            if (other.operation == record.operation && other.mutation != Some(record.mutation))
                || (other.mutation == Some(record.mutation) && other.operation != record.operation)
            {
                return Err(ReservationError::Storage(corrupt()));
            }
        }
        let mut found_outbox = false;
        for (key, bytes) in tx
            .scan_prefix(Table::EffectOutbox, &[])
            .map_err(ReservationError::Storage)?
        {
            let effect = decode_outbox(&key, &bytes).map_err(ReservationError::Storage)?;
            let same_mutation = effect.mutation == Some(record.mutation);
            let same_command = record.command.is_some() && effect.command == record.command;
            if same_mutation || same_command {
                if found_outbox || effect.mutation != Some(record.mutation) {
                    return Err(ReservationError::Storage(corrupt()));
                }
                validate_mutation_outbox(&record, &effect).map_err(ReservationError::Storage)?;
                found_outbox = true;
            } else if effect.key.operation == record.operation && effect.mutation.is_some() {
                return Err(ReservationError::Storage(corrupt()));
            }
        }
        if (matches!(
            record.phase,
            MutationPhase::DispatchArmed
                | MutationPhase::ProviderObserved
                | MutationPhase::RegistryCommitted
                | MutationPhase::NeedsReconciliation
        ) || record.phase == MutationPhase::NoChange && record.command.is_some())
            && !found_outbox
        {
            return Err(ReservationError::Storage(corrupt()));
        }
        return Ok(Reservation::Existing(existing));
    }
    let record = MutationRecord {
        mutation: input.mutation,
        operation: input.operation,
        subject: input.subject,
        authenticator: input.authenticator,
        expected_generation: input.expected_generation,
        phase: MutationPhase::Prepared,
        worker: None,
        epoch: None,
        command: None,
        recovery_ref: None,
        provider_observation_digest: None,
        terminal_outcome: None,
        plan: Some(input.plan.clone()),
    };
    let encoded = encode_mutation(&record).map_err(ReservationError::Storage)?;
    let lock_key = record.scope_key();
    if tx
        .get(Table::Mutations, input.mutation.as_bytes())
        .map_err(ReservationError::Storage)?
        .is_some()
        || tx
            .get(Table::MutationScopeLock, &lock_key)
            .map_err(ReservationError::Storage)?
            .is_some()
    {
        return Err(ReservationError::Storage(StorageError::GenerationConflict));
    }
    for (key, value) in tx
        .scan_prefix(Table::Mutations, &[])
        .map_err(ReservationError::Storage)?
    {
        let other = decode_mutation(&value).map_err(ReservationError::Storage)?;
        if key.as_slice() != other.mutation.as_bytes() {
            return Err(ReservationError::Storage(corrupt()));
        }
        if other.operation == input.operation {
            return Err(ReservationError::Storage(StorageError::GenerationConflict));
        }
        if other.phase != MutationPhase::Completed && other.scope_key() == record.scope_key() {
            // There is no blockage, but the journal is still alive: it cannot be considered a
            // free region and replaced by a new mutation.
            return Err(ReservationError::Storage(corrupt()));
        }
    }
    // The operation number may already belong to a validation that doesn't have a mutation
    // log entry. The new key should not be reassigned.
    for (key, value) in tx
        .scan_prefix(Table::Idempotency, &[])
        .map_err(ReservationError::Storage)?
    {
        let existing = decode_idempotency(&key, &value).map_err(ReservationError::Storage)?;
        if existing.operation == input.operation || existing.mutation == Some(input.mutation) {
            return Err(ReservationError::Storage(StorageError::GenerationConflict));
        }
    }
    crate::journal_store::check_current_scope_v3(tx, &record).map_err(ReservationError::Storage)?;
    let staged = crate::idempotency::stage(tx, idempotency)?;
    if staged != Reservation::Staged {
        return Err(ReservationError::Storage(corrupt()));
    }
    tx.put(Table::Mutations, record.mutation.as_bytes(), &encoded)
        .map_err(ReservationError::Storage)?;
    tx.put(
        Table::MutationScopeLock,
        &lock_key,
        record.mutation.as_bytes(),
    )
    .map_err(ReservationError::Storage)?;
    Ok(Reservation::Staged)
}
