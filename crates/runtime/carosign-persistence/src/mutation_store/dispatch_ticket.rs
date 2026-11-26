use crate::model::StorageError;
use crate::store::Table;

use super::read_view::ReadView;
use super::schema::{SCHEMA_V4, corrupt};

/// Checks in one picture the exact permitted shipment and its plan.
/// The ticket remains a one-time value in the calling layer; this request does not serialize
/// subsequent side effects with a competitive review.
pub fn validate_dispatch_ticket_v4(
    store: &dyn crate::store::MetadataStore,
    ticket: &crate::journal::ArmedMutationTicket,
    expected_plan: &carosign_core::operation::plan::MutationPlan,
) -> Result<(), StorageError> {
    use crate::model::MutationPhase;
    use crate::outbox::EffectPhase;
    let tx = store.begin_read()?;
    let meta = tx.scan_prefix(Table::Meta, &[])?;
    if meta.len() != 1 || meta[0].0 != crate::redb::backend::SCHEMA_KEY || meta[0].1 != SCHEMA_V4 {
        return Err(corrupt());
    }
    let view = ReadView { inner: &*tx };
    let snapshot = crate::recovery::read_mutation_snapshot_v3_in_snapshot(
        &view,
        ticket.mutation(),
        ticket.operation(),
    )?;
    let record = &snapshot.mutation;
    let effect = ticket.effect().ok_or_else(corrupt)?;
    if effect.spec != carosign_engine::EffectSpec::SendMutatingCommand
        || record.phase != MutationPhase::DispatchArmed
        || record.scope_key() != *ticket.scope()
        || record.worker != Some(ticket.worker())
        || record.epoch != Some(ticket.epoch())
        || record.command != Some(ticket.command())
        || snapshot.outbox.len() != 1
        || snapshot.outbox[0].phase != EffectPhase::Armed
        || snapshot.outbox[0].key.operation != ticket.operation()
        || snapshot.outbox[0].key.transition_index != effect.transition
        || snapshot.outbox[0].key.position.value() != effect.position
    {
        return Err(StorageError::GenerationConflict);
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
    if &plan != expected_plan {
        return Err(StorageError::GenerationConflict);
    }
    Ok(())
}
