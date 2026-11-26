use super::ownership::{check_idempotency_owner, check_journal_owner};
use super::record_error::corrupt;
use super::{
    decode_mutation, decode_outbox, encode_mutation, encode_outbox, validate_mutation_outbox,
};
use crate::model::{MutationPhase, StorageError};
use crate::mutation_plan::Transition;
use crate::outbox::EffectPhase;
use crate::store::{Table, WriteTransaction};
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::OpaqueProviderRef;
use carosign_core::ids::{CommandId, MutationId, OperationId, WorkerInstanceId};

/// Verified by a trusted boundary surveillance data from the provider command.
///
/// Digest is not an independent proof of the condition of the device.
#[derive(Clone, PartialEq, Eq)]
pub struct ObservedMutation {
    pub mutation: MutationId,
    pub operation: OperationId,
    pub command: CommandId,
    pub worker: WorkerInstanceId,
    pub epoch: DeviceEpoch,
    /// Address of steady delivery queue.
    pub effect: carosign_engine::transition::EffectKey,
    /// Observation digest obtained by a trusted calling code.
    pub observation_digest: [u8; 32],
    /// A link for subsequent reconciliation, if it is communicated.
    pub recovery_ref: Option<OpaqueProviderRef>,
}

/// Prepares surveillance and queue updates in one v3 transaction.
///
/// The caller has to check the event against the session and the plan. Success means only
/// prepared records, not stability, not proven mutation.
/// The new registration link is not stored here; its retention requires a separate, verified
/// ledger path.
pub fn stage_observation(
    tx: &mut dyn WriteTransaction,
    input: &ObservedMutation,
) -> Result<(), StorageError> {
    stage_observation_inner(tx, input, false)
}

pub(crate) fn stage_recovery_observation(
    tx: &mut dyn WriteTransaction,
    input: &ObservedMutation,
) -> Result<(), StorageError> {
    stage_observation_inner(tx, input, true)
}

fn stage_observation_inner(
    tx: &mut dyn WriteTransaction,
    input: &ObservedMutation,
    recovering: bool,
) -> Result<(), StorageError> {
    let bytes = tx
        .get(Table::Mutations, input.mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let mut record = decode_mutation(&bytes)?;
    if record.mutation != input.mutation
        || record.operation != input.operation
        || record.worker != Some(input.worker)
        || record.epoch != Some(input.epoch)
        || record.command != Some(input.command)
        || input.effect.operation != input.operation
    {
        return Err(corrupt());
    }
    let expected_effect = match record.phase {
        MutationPhase::DispatchArmed => EffectPhase::Armed,
        MutationPhase::ProviderObserved if recovering => EffectPhase::Observed,
        MutationPhase::NeedsReconciliation if recovering => EffectPhase::ReconciliationRequired,
        _ => return Err(StorageError::GenerationConflict),
    };
    if tx
        .get(Table::MutationScopeLock, &record.scope_key())?
        .as_deref()
        != Some(input.mutation.as_bytes().as_slice())
    {
        return Err(corrupt());
    }
    check_journal_owner(tx, &record)?;
    let key = crate::redb::codec::outbox_key(
        input.effect.operation.as_bytes(),
        input.effect.transition_index.value(),
        input.effect.position.value(),
    );
    let bytes = tx.get(Table::EffectOutbox, &key)?.ok_or_else(corrupt)?;
    let mut outbox = decode_outbox(&key, &bytes)?;
    validate_mutation_outbox(&record, &outbox)?;
    if outbox.phase != expected_effect {
        return Err(StorageError::GenerationConflict);
    }
    for (other_key, other_bytes) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let other = decode_outbox(&other_key, &other_bytes)?;
        if other_key != key
            && (other.mutation == Some(input.mutation) || other.command == Some(input.command))
        {
            return Err(corrupt());
        }
    }
    record.phase = MutationPhase::ProviderObserved;
    record.provider_observation_digest = Some(input.observation_digest);
    if input.recovery_ref.is_some() {
        record.recovery_ref = input.recovery_ref.clone();
    }
    outbox.phase = EffectPhase::Observed;
    let record_bytes = encode_mutation(&record)?;
    let outbox_bytes = encode_outbox(&outbox)?;
    tx.put(Table::Mutations, input.mutation.as_bytes(), &record_bytes)?;
    tx.put(Table::EffectOutbox, &key, &outbox_bytes)?;
    Ok(())
}

pub(crate) fn stage_enrollment_rollback(
    tx: &mut dyn WriteTransaction,
    input: &ObservedMutation,
) -> Result<(), StorageError> {
    let bytes = tx
        .get(Table::Mutations, input.mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let mut record = decode_mutation(&bytes)?;
    let transition = record.plan.as_ref().ok_or_else(corrupt)?.transition;
    if !matches!(
        transition,
        Transition::EnrollNew | Transition::EnrollReplace
    ) {
        return Err(StorageError::GenerationConflict);
    }
    check_idempotency_owner(tx, &record)?;
    crate::journal_store::check_current_scope_v3(tx, &record)?;
    stage_observation(tx, input)?;
    record.phase = MutationPhase::NoChange;
    record.provider_observation_digest = Some(input.observation_digest);
    if input.recovery_ref.is_some() {
        record.recovery_ref = input.recovery_ref.clone();
    }
    tx.put(
        Table::Mutations,
        input.mutation.as_bytes(),
        &encode_mutation(&record)?,
    )
}
