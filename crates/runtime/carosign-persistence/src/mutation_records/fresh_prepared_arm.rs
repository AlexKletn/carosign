use super::durable_arm::arm_durable_v3;
use super::record_error::corrupt;
use super::{DurableArmFailure, FreshPreparationFailure, stage_prepared};
use crate::idempotency::Reservation;
use crate::journal::{ArmMutation, PrepareMutation};
use crate::model::{IdempotencyRecord, StorageError};
use crate::mutation_plan::Scope;
use crate::store::Table;
use carosign_core::ids::{MutationId, OperationId};

/// A non-repeatable internal possession of a newly recorded training.
/// It is not a power: the caller is obliged to separately check the actor, live issuance,
/// recall and deadline.
#[allow(dead_code)]
pub(crate) struct FreshPreparedArm<'a> {
    store: &'a dyn crate::store::MetadataStore,
    operation: OperationId,
    mutation: MutationId,
    plan: carosign_core::operation::plan::MutationPlan,
}

impl FreshPreparedArm<'_> {
    /// Consumes the sole right to attempt, including failure prior to commit and unknown
    /// outcome commit.Renewal after restart does not give this opportunity away from a single
    /// Prepared record.
    #[allow(dead_code)]
    pub(crate) fn arm(
        self,
        input: &ArmMutation,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<crate::journal::ArmedMutationTicket, DurableArmFailure> {
        if input.mutation != self.mutation {
            return Err(DurableArmFailure::BeforeCommit(
                StorageError::GenerationConflict,
            ));
        }
        arm_durable_v3(self.store, self.operation, input, effect, &self.plan)
    }
}

/// It gives the right to try only to the same owner who has confirmed the new training.
/// Idempotent repetition and unknown commit do not create rights.
/// The source of authority and normalized digest checks the composition of the service.
#[allow(dead_code)]
pub(crate) fn prepare_fresh_arm<'a>(
    store: &'a dyn crate::store::MetadataStore,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
) -> Result<FreshPreparedArm<'a>, FreshPreparationFailure> {
    let plan = input
        .plan
        .to_core(Scope {
            subject: input.subject,
            authenticator: input.authenticator,
        })
        .map_err(|_| FreshPreparationFailure::Storage(corrupt()))?;
    let mut tx = store
        .begin_write()
        .map_err(FreshPreparationFailure::Storage)?;
    let meta = tx
        .scan_prefix(Table::Meta, &[])
        .map_err(FreshPreparationFailure::Storage)?;
    if meta.len() != 1
        || meta[0].0 != crate::redb::backend::SCHEMA_KEY
        || meta[0].1 != crate::redb::backend::SCHEMA_V3
    {
        return Err(FreshPreparationFailure::Storage(corrupt()));
    }
    match stage_prepared(&mut *tx, input, idempotency)
        .map_err(FreshPreparationFailure::Reservation)?
    {
        Reservation::Staged => {}
        Reservation::Existing(_) => return Err(FreshPreparationFailure::Existing),
    }
    tx.commit_durable()
        .map_err(FreshPreparationFailure::CommitUnknown)?;
    Ok(FreshPreparedArm {
        store,
        operation: input.operation,
        mutation: input.mutation,
        plan,
    })
}
