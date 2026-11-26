use carosign_core::ids::{MutationId, OperationId};

use crate::{
    enrollment_codec::{EnrollmentRecord, EnrollmentStatus},
    model::{MutationPhase, MutationRecord, StorageError},
    mutation_plan::{Scope, StoredBindingKind},
    mutation_records::{ObservedMutation, RegistryUpdate, decode_mutation, encode_mutation},
    projection::RegistryEntry,
    store::{MetadataStore, Table, WriteTransaction},
};

use super::{
    DurableRegistryFailure, RecoveryEvidence,
    read_view::ReadView,
    record_view::RecordView,
    schema::{SCHEMA_V4, corrupt},
};

pub fn recover_armed_v4(
    store: &dyn MetadataStore,
    mutation: MutationId,
    operation: OperationId,
    verify: impl FnOnce(&crate::recovery::MutationSnapshot) -> Option<RecoveryEvidence>,
    confirm_cleanup: impl FnOnce(&MutationRecord) -> bool,
) -> Result<bool, DurableRegistryFailure> {
    let failed = DurableRegistryFailure::BeforeCommit;
    let read = store.begin_read().map_err(failed)?;
    let meta = read.scan_prefix(Table::Meta, &[]).map_err(failed)?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(failed(corrupt()));
    }
    let snapshot = crate::recovery::read_mutation_snapshot_v3_in_snapshot(
        &ReadView { inner: &*read },
        mutation,
        operation,
    )
    .map_err(failed)?;
    drop(read);
    let record = &snapshot.mutation;
    if !matches!(
        record.phase,
        MutationPhase::DispatchArmed
            | MutationPhase::ProviderObserved
            | MutationPhase::RegistryCommitted
            | MutationPhase::NeedsReconciliation
            | MutationPhase::NoChange
    ) || record.command.is_none()
    {
        return Ok(false);
    }
    let Some(evidence) = verify(&snapshot) else {
        return Ok(false);
    };
    let Some(effect) = snapshot.outbox.first() else {
        return Err(failed(corrupt()));
    };
    if evidence.mutation != mutation
        || evidence.operation != operation
        || Some(evidence.command) != record.command
        || Some(evidence.worker) != record.worker
        || Some(evidence.epoch) != record.epoch
        || evidence.digest == [0; 32]
        || evidence.scope
            != (Scope {
                subject: record.subject,
                authenticator: record.authenticator,
            })
    {
        return Ok(false);
    }
    let Some((desired, new)) = projection(record, &evidence.enrollments) else {
        return Ok(false);
    };
    let mut tx = store.begin_write().map_err(failed)?;
    let result = {
        let view: &mut dyn WriteTransaction = &mut RecordView { inner: &mut *tx };
        stage_recovery(view, record, effect.key, &evidence, desired, new)
            .and_then(|()| journal_record(view, mutation))
    };
    let committed = match result {
        Ok(record) => record,
        Err(error) => {
            tx.abort();
            return if error == StorageError::GenerationConflict {
                Ok(false)
            } else {
                Err(failed(error))
            };
        }
    };
    tx.commit_durable()
        .map_err(DurableRegistryFailure::CommitUnknown)?;
    if !confirm_cleanup(&committed) {
        return Ok(false);
    }
    let mut tx = store.begin_write().map_err(failed)?;
    let finish = {
        let view: &mut dyn WriteTransaction = &mut RecordView { inner: &mut *tx };
        stage_recovery(view, &committed, effect.key, &evidence, desired, new)
            .and_then(|()| crate::mutation_records::stage_completed(view, mutation, operation))
    };
    if let Err(error) = finish {
        tx.abort();
        return if error == StorageError::GenerationConflict {
            Ok(false)
        } else {
            Err(failed(error))
        };
    }
    tx.commit_durable()
        .map_err(DurableRegistryFailure::CommitUnknown)?;
    Ok(true)
}

fn journal_record(
    tx: &dyn WriteTransaction,
    mutation: MutationId,
) -> Result<MutationRecord, StorageError> {
    let bytes = tx
        .get(Table::Mutations, mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    decode_mutation(&bytes)
}

fn projection<'a>(
    record: &MutationRecord,
    observed: &'a [EnrollmentRecord],
) -> Option<(bool, Option<&'a EnrollmentRecord>)> {
    let plan = record.plan.as_ref()?;
    if observed.len() > plan.pre_bindings.len() + 1 {
        return None;
    }
    let entries: Vec<_> = observed.iter().map(RegistryEntry::from_record).collect();
    if crate::projection::check_scope(plan, &entries).is_ok() {
        return Some((false, None));
    }
    let mut target = plan.clone();
    target
        .pre_bindings
        .retain(|entry| Some(entry.enrollment) != plan.old_target);
    let new = observed
        .iter()
        .find(|entry| Some(entry.enrollment.id) == plan.new_enrollment);
    if plan.new_enrollment.is_some() != new.is_some() {
        return None;
    }
    if let Some(entry) = new
        && (entry.subject != record.subject
            || entry.enrollment.authenticator != record.authenticator
            || entry.enrollment.generation.value() != plan.new_generation
            || entry.status != EnrollmentStatus::Active
            || entry.binding_kind != StoredBindingKind::ProviderOpaque
            || Some(&entry.label) != plan.requested_label.as_ref())
    {
        return None;
    }
    let unchanged: Vec<_> = entries
        .into_iter()
        .filter(|entry| Some(entry.enrollment.id) != plan.new_enrollment)
        .collect();
    if unchanged.len() + usize::from(new.is_some()) != observed.len()
        || crate::projection::check_scope(&target, &unchanged).is_err()
    {
        return None;
    }
    Some((true, new))
}

fn stage_recovery(
    tx: &mut dyn WriteTransaction,
    record: &MutationRecord,
    effect: carosign_engine::transition::EffectKey,
    evidence: &RecoveryEvidence,
    desired: bool,
    new: Option<&EnrollmentRecord>,
) -> Result<(), StorageError> {
    if journal_record(tx, record.mutation)? != *record {
        return Err(StorageError::GenerationConflict);
    }
    if matches!(
        record.phase,
        MutationPhase::RegistryCommitted | MutationPhase::NoChange
    ) {
        let current = crate::journal_store::read_indexed_scope_v3(tx, record)?;
        let matches_scope = current.len() == evidence.enrollments.len()
            && evidence
                .enrollments
                .iter()
                .all(|entry| current.contains(&RegistryEntry::from_record(entry)));
        let matches_new = if let Some(new) = new {
            tx.get(Table::Enrollments, new.enrollment.id.as_bytes())?
                .is_some_and(|bytes| crate::enrollment_codec::decode_v3(&bytes).as_ref() == Ok(new))
        } else {
            true
        };
        return if desired == (record.phase == MutationPhase::RegistryCommitted)
            && matches_scope
            && matches_new
        {
            Ok(())
        } else {
            Err(StorageError::GenerationConflict)
        };
    }
    crate::journal_store::check_current_scope_v3(tx, record)?;
    crate::mutation_records::stage_recovery_observation(
        tx,
        &ObservedMutation {
            mutation: evidence.mutation,
            operation: evidence.operation,
            command: evidence.command,
            worker: evidence.worker,
            epoch: evidence.epoch,
            effect,
            observation_digest: evidence.digest,
            recovery_ref: None,
        },
    )?;
    if desired {
        return crate::mutation_records::stage_registry_update(
            tx,
            &RegistryUpdate {
                mutation: evidence.mutation,
                operation: evidence.operation,
                command: evidence.command,
                worker: evidence.worker,
                epoch: evidence.epoch,
                observation_digest: evidence.digest,
                new_record: new,
            },
        );
    }
    let mut record = record.clone();
    record.phase = MutationPhase::NoChange;
    record.provider_observation_digest = Some(evidence.digest);
    tx.put(
        Table::Mutations,
        record.mutation.as_bytes(),
        &encode_mutation(&record)?,
    )
}

pub fn armed_recovery_targets_v4(
    store: &dyn MetadataStore,
) -> Result<Vec<(MutationId, OperationId)>, StorageError> {
    use crate::store::ReadTransaction;
    let read = store.begin_read()?;
    let scan = super::scan_prepared_v4_snapshot(&*read)?;
    let view = ReadView { inner: &*read };
    scan.scan
        .needing_reconciliation
        .into_iter()
        .chain(scan.scan.needs_decision)
        .filter_map(|mutation| {
            let record = view
                .get(Table::Mutations, mutation.as_bytes())
                .and_then(|bytes| decode_mutation(&bytes.ok_or_else(corrupt)?));
            match record {
                Ok(record) if record.command.is_none() => None,
                Ok(record) => Some(Ok((mutation, record.operation))),
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}
