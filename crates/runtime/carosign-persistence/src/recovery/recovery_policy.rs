use super::{
    MetadataStore, MutationId, MutationPhase, MutationRecord, RecoveryDecision, StartupScan,
    StorageError,
};

/// One-record decision.
pub fn decide_for(record: &MutationRecord) -> RecoveryDecision {
    let Some(plan) = &record.plan else {
        return RecoveryDecision::NeedsDecision;
    };
    if record.validate().is_err()
        || plan
            .validate(crate::mutation_plan::Scope {
                subject: record.subject,
                authenticator: record.authenticator,
            })
            .is_err()
    {
        return RecoveryDecision::NeedsDecision;
    }
    let complete_binding =
        record.worker.is_some() && record.epoch.is_some() && record.command.is_some();
    if matches!(
        record.phase,
        MutationPhase::DispatchArmed
            | MutationPhase::ProviderObserved
            | MutationPhase::RegistryCommitted
    ) && !complete_binding
    {
        return RecoveryDecision::NeedsDecision;
    }
    match record.phase {
        MutationPhase::Prepared => RecoveryDecision::CloseAsNoChange,
        MutationPhase::DispatchArmed
        | MutationPhase::ProviderObserved
        | MutationPhase::RegistryCommitted
        | MutationPhase::NeedsReconciliation => RecoveryDecision::BlockAndReconcile,
        // This model does not contain mandatory terminal_outcomeA historical record cannot be
        // considered complete or automatically corrected.
        MutationPhase::Completed | MutationPhase::NoChange => RecoveryDecision::NeedsDecision,
    }
}

/// Does the recovery allow the changing command to be repeated?
///
/// ⚠️ The answer is always `false`The question is asked so that the rule can be seen in its
/// place and cannot be circumvented by a single line.
pub fn may_replay(record: &MutationRecord) -> bool {
    let _ = record;
    false
}

/// List of areas to be blocked before service is ready.
pub fn scopes_to_block(store: &dyn MetadataStore) -> Result<Vec<MutationId>, StorageError> {
    let scan = StartupScan::scan(store)?;
    let mut blocked = scan.needing_reconciliation;
    blocked.extend(scan.safe_to_close);
    blocked.extend(scan.needs_decision);
    blocked.sort();
    blocked.dedup();
    Ok(blocked)
}
