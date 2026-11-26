use super::ownership::{check_idempotency_owner, check_journal_owner};
use super::record_error::corrupt;
use super::{decode_mutation, decode_outbox, encode_mutation, validate_mutation_outbox};
use crate::model::{MutationPhase, StorageError};
use crate::mutation_plan::{Scope, StoredBindingKind, Transition};
use crate::outbox::EffectPhase;
use crate::store::{Table, WriteTransaction};
use carosign_core::device::DeviceEpoch;
use carosign_core::ids::{CommandId, MutationId, OperationId, WorkerInstanceId};

/// Service composition-verified result of the change, and the provider's observation must be
/// pre-session-tested and persistently recorded.
pub struct RegistryUpdate<'a> {
    pub mutation: MutationId,
    pub operation: OperationId,
    pub command: CommandId,
    pub worker: WorkerInstanceId,
    pub epoch: DeviceEpoch,
    pub observation_digest: [u8; 32],
    /// New entry when created or replaced; no deletion.
    pub new_record: Option<&'a crate::enrollment_codec::EnrollmentRecord>,
}

/// Prepares the atomic application of the verified result to the blocked area, the caller
/// confirms the origin of the observation and fixes the transaction; the successful staging
/// itself does not prove stability.
pub fn stage_registry_update(
    tx: &mut dyn WriteTransaction,
    input: &RegistryUpdate<'_>,
) -> Result<(), StorageError> {
    use crate::enrollment_codec::EnrollmentStatus;
    let bytes = tx
        .get(Table::Mutations, input.mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let mut record = decode_mutation(&bytes)?;
    if record.mutation != input.mutation
        || record.operation != input.operation
        || record.command != Some(input.command)
        || record.worker != Some(input.worker)
        || record.epoch != Some(input.epoch)
        || record.provider_observation_digest != Some(input.observation_digest)
    {
        return Err(corrupt());
    }
    if record.phase != MutationPhase::ProviderObserved {
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
    let mut observed = false;
    for (key, bytes) in tx.scan_prefix(Table::EffectOutbox, &[])? {
        let entry = decode_outbox(&key, &bytes)?;
        if entry.mutation == Some(input.mutation) || entry.command == Some(input.command) {
            validate_mutation_outbox(&record, &entry)?;
            if observed || entry.phase != EffectPhase::Observed {
                return Err(corrupt());
            }
            observed = true;
        }
    }
    if !observed {
        return Err(corrupt());
    }
    let entries = crate::journal_store::read_current_scope_v3(tx, &record)?;
    let plan = record.plan.as_ref().ok_or_else(corrupt)?;
    plan.validate(Scope {
        subject: record.subject,
        authenticator: record.authenticator,
    })
    .map_err(|_| corrupt())?;
    crate::projection::check_scope(plan, &entries).map_err(|_| StorageError::GenerationConflict)?;
    let new = match plan.transition {
        Transition::Remove if input.new_record.is_none() => None,
        Transition::EnrollNew | Transition::EnrollReplace => {
            let new = input.new_record.ok_or_else(corrupt)?;
            if new.subject != record.subject
                || new.enrollment.authenticator != record.authenticator
                || plan.requested_label.as_ref() != Some(&new.label)
                || Some(new.enrollment.id) != plan.new_enrollment
                || new.enrollment.generation.value() != plan.new_generation
                || new.status != EnrollmentStatus::Active
                || new.binding_kind != StoredBindingKind::ProviderOpaque
                || tx
                    .get(Table::Enrollments, new.enrollment.id.as_bytes())?
                    .is_some()
                || tx
                    .get(
                        Table::EnrollmentIndex,
                        &crate::redb::codec::enrollment_index_key(new),
                    )?
                    .is_some()
            {
                return Err(StorageError::GenerationConflict);
            }
            Some(new)
        }
        _ => return Err(corrupt()),
    };
    if let Some(old) = plan.old_target {
        let entry = entries
            .iter()
            .find(|entry| entry.enrollment == old)
            .ok_or_else(corrupt)?;
        if entry.kind != StoredBindingKind::ProviderOpaque {
            return Err(StorageError::Unavailable);
        }
        let mut key = [0u8; crate::limits::ENROLLMENT_INDEX_KEY_LEN];
        key[..crate::limits::SCOPE_LOCK_KEY_LEN].copy_from_slice(&record.scope_key());
        key[crate::limits::SCOPE_LOCK_KEY_LEN..].copy_from_slice(old.id.as_bytes());
        if tx.get(Table::EnrollmentIndex, &key)?.as_deref() != Some(old.id.as_bytes().as_slice()) {
            return Err(corrupt());
        }
        tx.delete(Table::EnrollmentIndex, &key)?;
        tx.delete(Table::Enrollments, old.id.as_bytes())?;
    }
    if let Some(new) = new {
        tx.put(
            Table::Enrollments,
            new.enrollment.id.as_bytes(),
            &crate::enrollment_codec::encode_v3(new).map_err(|_| corrupt())?,
        )?;
        tx.put(
            Table::EnrollmentIndex,
            &crate::redb::codec::enrollment_index_key(new),
            new.enrollment.id.as_bytes(),
        )?;
    }
    record.phase = MutationPhase::RegistryCommitted;
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record)?,
    )?;
    Ok(())
}
