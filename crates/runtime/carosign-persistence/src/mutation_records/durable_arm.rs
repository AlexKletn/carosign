use super::record_error::corrupt;
use super::{DurableArmFailure, decode_mutation, stage_arm};
use crate::journal::ArmMutation;
use crate::model::StorageError;
use crate::mutation_plan::Scope;
use crate::store::Table;
use carosign_core::ids::OperationId;

#[allow(dead_code)]
pub(super) fn arm_durable_v3(
    store: &dyn crate::store::MetadataStore,
    operation: OperationId,
    input: &ArmMutation,
    effect: carosign_engine::PlannedEffect,
    expected_plan: &carosign_core::operation::plan::MutationPlan,
) -> Result<crate::journal::ArmedMutationTicket, DurableArmFailure> {
    arm_durable_for_schema(
        store,
        operation,
        input,
        effect,
        expected_plan,
        crate::redb::backend::SCHEMA_V3,
    )
}

fn arm_durable_for_schema(
    store: &dyn crate::store::MetadataStore,
    operation: OperationId,
    input: &ArmMutation,
    effect: carosign_engine::PlannedEffect,
    expected_plan: &carosign_core::operation::plan::MutationPlan,
    expected_schema: &[u8],
) -> Result<crate::journal::ArmedMutationTicket, DurableArmFailure> {
    let mut tx = store.begin_write()?;
    if tx
        .get(Table::Meta, crate::redb::backend::SCHEMA_KEY)?
        .as_deref()
        != Some(expected_schema)
    {
        return Err(StorageError::Corrupt {
            what: "неверная схема вооружения изменения",
        }
        .into());
    }
    // An unknown meta can mean an incomplete migration. One v3 marker does not allow for
    // changing the log or issuing a new ticket.
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY {
        return Err(StorageError::Corrupt {
            what: "неизвестное состояние метаданных вооружения изменения",
        }
        .into());
    }
    let bytes = tx
        .get(Table::Mutations, input.mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let record = decode_mutation(&bytes)?;
    if record.mutation != input.mutation || record.operation != operation {
        return Err(corrupt().into());
    }
    let stored_plan = record.plan.as_ref().ok_or_else(corrupt)?;
    let actual_plan = stored_plan
        .to_core(Scope {
            subject: record.subject,
            authenticator: record.authenticator,
        })
        .map_err(|_| corrupt())?;
    if &actual_plan != expected_plan {
        return Err(StorageError::GenerationConflict.into());
    }
    let scope = record.scope_key();
    stage_arm(&mut *tx, input, effect)?;
    tx.commit_durable()
        .map_err(DurableArmFailure::CommitUnknown)?;
    Ok(crate::journal::ArmedMutationTicket::new(
        input.mutation,
        operation,
        input.command,
        input.worker,
        input.epoch,
        scope,
        Some(effect),
    ))
}
