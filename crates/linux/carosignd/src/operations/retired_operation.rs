use carosign_core::{
    ids::OperationId,
    operation::context::{BootDeadline, BootInstant},
};
use carosign_engine::OperationPhase;

use super::{
    CleanupStage, Entry, LiveOperationStatus, OPERATIONS_MAX, OperationRegistry,
    TerminalRecordStage, VerificationBindingStage, VerificationDispatchStage,
};

pub(crate) const VERIFICATION_KEYS_MAX: usize = 4096;
const RETENTION_NS: u64 = 300_000_000_000;

pub(super) struct RetiredOperation {
    pub(super) status: LiveOperationStatus,
    expires: BootDeadline,
}

impl OperationRegistry {
    pub(crate) fn is_completed(&self, operation: OperationId) -> bool {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        entries.retired.contains_key(&operation)
            || entries.get(&operation).is_some_and(|entry| entry.terminal)
    }

    pub(crate) fn retire_completed(&self, now: BootInstant) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let expired: std::collections::BTreeSet<_> = entries
            .retired
            .iter()
            .filter(|(_, retired)| {
                retired.expires.boot() == now.boot() && retired.expires.is_expired_at(now)
            })
            .map(|(id, _)| *id)
            .collect();
        if !expired.is_empty() {
            entries.retired.retain(|id, _| !expired.contains(id));
            entries
                .idempotency
                .retain(|_, (_, id)| !expired.contains(id));
        }

        if entries.len() < OPERATIONS_MAX || entries.retired.len() >= VERIFICATION_KEYS_MAX {
            return;
        }
        let remaining = VERIFICATION_KEYS_MAX.saturating_sub(entries.retired.len());
        let candidates: Vec<_> = entries
            .operations
            .iter()
            .filter(|(id, entry)| locally_drained(self, entry, **id))
            .take(remaining)
            .map(|(id, _)| *id)
            .collect();
        for id in candidates {
            let Some(entry) = entries.operations.remove(&id) else {
                continue;
            };
            let status = LiveOperationStatus::from_entry(&entry);
            let expires = BootDeadline::new(
                entry.deadline.boot(),
                entry.deadline.instant_ns().saturating_add(RETENTION_NS),
            );
            entries
                .retired
                .insert(id, RetiredOperation { status, expires });
            for usage in entries.connections.values_mut() {
                usage.operations.remove(&id);
            }
        }
    }

    pub(crate) fn publish_completed_mutation(&self, operation: OperationId) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(entry) = entries.get_mut(&operation) else {
            return;
        };
        if !entry.is_mutation {
            return;
        }
        let mut hub = entry
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if hub
            .publish(
                OperationPhase::Terminal,
                carosign_runtime::operation::events::EventKind::Terminal,
            )
            .is_ok()
        {
            entry.terminal = true;
        }
        if entry.terminal {
            drop(hub);
            for usage in entries.connections.values_mut() {
                usage.operations.remove(&operation);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn retirement_locally_drained(&self, id: OperationId) -> bool {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        entries
            .idempotency
            .values()
            .any(|(_, operation)| *operation == id)
            && entries
                .get(&id)
                .is_some_and(|entry| locally_drained(self, entry, id))
    }
}

fn locally_drained(registry: &OperationRegistry, entry: &Entry, id: OperationId) -> bool {
    let Some(owned) = entry.operation.as_ref() else {
        return false;
    };
    if !entry.is_mutation
        && (owned.cleanup_stage != CleanupStage::Released
            || !matches!(owned.terminal_record_stage, TerminalRecordStage::Recorded { outcome, .. } if Some(outcome) == owned.actor.state().common.terminal))
    {
        return false;
    }
    if !entry.terminal
        || !entry.pending_control.is_empty()
        || entry.retained_delivery.is_some()
        || entry.positive_reservation.is_some()
        || matches!(
            entry.verification_binding,
            VerificationBindingStage::Binding | VerificationBindingStage::Ready { .. }
        )
        || matches!(
            entry.verification_dispatch,
            VerificationDispatchStage::Sending | VerificationDispatchStage::OutcomeUnknown
        )
        || entry.verification_dispatch_step.is_some()
        || entry.matched_evidence_step.is_some()
        || entry.matched_enrollment.is_some()
        || entry.evidence_evaluation_step.is_some()
        || entry.owner_revalidation_step.is_some()
        || entry.evidence_destroy_step.is_some()
        || entry.mutation_binding_step.is_some()
        || owned.deferred_control_step.is_some()
        || !owned.terminal_disconnects.is_empty()
        || owned.actor.has_work()
        || owned.actor.has_control_work()
        || owned.lease.is_some()
        || owned.actor.state().common.lease.is_some()
        || !owned.actor.state().common.pending_effects.is_empty()
        || owned.actor.state().common.phase != carosign_engine::OperationPhase::Terminal
        || std::sync::Arc::strong_count(&entry.events) != 2
        || registry.evidence.contains_operation(id)
    {
        return false;
    }
    entry
        .events
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_drained_terminal()
}
