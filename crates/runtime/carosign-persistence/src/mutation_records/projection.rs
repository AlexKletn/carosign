use super::decode_mutation;
use super::ownership::{check_idempotency_owner, check_journal_owner};
use super::record_error::corrupt;
use crate::model::{MutationPhase, StorageError};
use crate::mutation_plan::{Scope, StoredBindingKind};
use crate::store::{Table, WriteTransaction};
use carosign_core::ids::{MutationId, OperationId};

/// Builds a projection of registration from the plan and registry under the area lock.
///
/// The caller is obliged to check the authority and tie the result to the current worker
/// and era. The result does not allow sending and does not survive the arm: after it,
/// reassembly and sending are prohibited.
pub fn enrollment_projection(
    tx: &dyn WriteTransaction,
    mutation: MutationId,
    operation: OperationId,
) -> Result<crate::projection::Projection, StorageError> {
    let bytes = tx
        .get(Table::Mutations, mutation.as_bytes())?
        .ok_or_else(corrupt)?;
    let record = decode_mutation(&bytes)?;
    if record.mutation != mutation || record.operation != operation {
        return Err(corrupt());
    }
    if record.phase != MutationPhase::Prepared {
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
    let entries = crate::journal_store::read_current_scope_v3(tx, &record)?;
    let plan = record.plan.as_ref().ok_or_else(corrupt)?;
    let projection = crate::projection::build(
        plan,
        Scope {
            subject: record.subject,
            authenticator: record.authenticator,
        },
        &entries,
    )
    .map_err(|_| StorageError::GenerationConflict)?;
    if let crate::projection::ProjectionChange::Replace { old, .. } = &projection.change
        && old.kind == StoredBindingKind::ScopedVault
    {
        // There is no approved limited storage port yet.
        return Err(StorageError::Unavailable);
    }
    Ok(projection)
}
