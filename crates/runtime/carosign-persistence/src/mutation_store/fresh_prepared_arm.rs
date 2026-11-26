use crate::idempotency::Reservation;
use crate::journal::{ArmMutation, PrepareMutation};
use crate::model::{IdempotencyRecord, StorageError};
use crate::store::{Table, WriteTransaction};

use super::arm::stage_arm_v4;
use super::preparation::stage_prepared_v4;
use super::record_view::RecordView;
use super::schema::{SCHEMA_V4, corrupt};

/// Unrepeated possession of fresh v4 confirmed preparation.
///
/// It’s an internal building block: it doesn’t confirm credentials, live issuance or recall,
/// and it’s not recovered from the base after a restart.
#[allow(dead_code)]
pub struct FreshPreparedArmV4<'a> {
    store: &'a dyn crate::store::MetadataStore,
    operation: carosign_core::ids::OperationId,
    mutation: carosign_core::ids::MutationId,
    plan: carosign_core::operation::plan::MutationPlan,
}

/// It records a new training and only after confirmation gives possession.
/// Unknown outcome of fixation, repetition and existing record of possession do not give.
#[allow(dead_code)]
pub fn prepare_fresh_arm_v4<'a>(
    store: &'a dyn crate::store::MetadataStore,
    input: &PrepareMutation,
    idempotency: &IdempotencyRecord,
) -> Result<FreshPreparedArmV4<'a>, crate::mutation_records::FreshPreparationFailure> {
    use crate::mutation_records::FreshPreparationFailure as Failure;
    let plan = input
        .plan
        .to_core(crate::mutation_plan::Scope {
            subject: input.subject,
            authenticator: input.authenticator,
        })
        .map_err(|_| Failure::Storage(corrupt()))?;
    let mut tx = store.begin_write().map_err(Failure::Storage)?;
    match stage_prepared_v4(&mut *tx, input, idempotency).map_err(Failure::Reservation)? {
        Reservation::Staged => {}
        Reservation::Existing(_) => return Err(Failure::Existing),
    }
    tx.commit_durable().map_err(Failure::CommitUnknown)?;
    Ok(FreshPreparedArmV4 {
        store,
        operation: input.operation,
        mutation: input.mutation,
        plan,
    })
}

impl FreshPreparedArmV4<'_> {
    /// Consumes opportunity even in case of an error or an unknown outcome commit.
    /// Verifications of authority and live issuance remain with the trusted owner.
    #[allow(dead_code)]
    pub fn arm(
        self,
        input: &ArmMutation,
        effect: carosign_engine::PlannedEffect,
    ) -> Result<crate::journal::ArmedMutationTicket, crate::mutation_records::DurableArmFailure>
    {
        use crate::mutation_records::DurableArmFailure as Failure;
        if input.mutation != self.mutation {
            return Err(Failure::BeforeCommit(StorageError::GenerationConflict));
        }
        let mut tx = self.store.begin_write()?;
        let meta = tx.scan_prefix(Table::Meta, &[])?;
        if meta.len() != 1
            || meta[0].0 != crate::redb::backend::SCHEMA_KEY
            || meta[0].1 != SCHEMA_V4
        {
            return Err(Failure::BeforeCommit(corrupt()));
        }
        let view = RecordView { inner: &mut *tx };
        let raw = view
            .get(Table::Mutations, self.mutation.as_bytes())?
            .ok_or_else(corrupt)?;
        let record = crate::mutation_records::decode_mutation(&raw)?;
        if record.mutation != self.mutation || record.operation != self.operation {
            return Err(Failure::BeforeCommit(corrupt()));
        }
        let plan = record
            .plan
            .as_ref()
            .ok_or_else(corrupt)?
            .to_core(crate::mutation_plan::Scope {
                subject: record.subject,
                authenticator: record.authenticator,
            })
            .map_err(|_| corrupt())?;
        if plan != self.plan {
            return Err(Failure::BeforeCommit(StorageError::GenerationConflict));
        }
        let scope = record.scope_key();
        stage_arm_v4(&mut *tx, input, effect)?;
        tx.commit_durable().map_err(Failure::CommitUnknown)?;
        Ok(crate::journal::ArmedMutationTicket::new(
            input.mutation,
            self.operation,
            input.command,
            input.worker,
            input.epoch,
            scope,
            Some(effect),
        ))
    }
}
