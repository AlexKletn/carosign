use crate::input::EffectSpec;
use crate::transition::OperationPhase;

/// Lines of table 04 - for reconciliation of the document and the code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MutationStep {
    /// `New` + `ManageGrantValid` → `Preparing`, `PersistIntent`.
    Preparing,
    /// `Preparing` + `IntentDurable` → `WaitingLease`, `AcquireResources`.
    WaitingLease,
    /// `WaitingLease` + `LeaseGranted` → `Preflight`, `RevalidateGrant`.
    Preflight,
    /// `Preflight` + `GrantValid` → `Arming`, `PersistDispatchArmed`.
    Arming,
    /// `Arming` + `DispatchArmedDurable` → `InFlight`, `SendMutatingCommand`.
    InFlight,
    /// `InFlight` + `ProviderCompleted` → `ValidatingChange`.
    ValidatingChange,
    /// `ValidatingChange` + `ChangeConfirmed` → `Committing`.
    Committing,
    /// `Committing` + `MetadataDurable` → `Finalizing`.
    Finalizing,
    /// `Finalizing` + `CleanupConfirmed` → `Terminal`.
    Terminal,
    /// Reconciliation after the cliff (B-01, B-02).
    Reconciling,
}

/// Table row phase.
pub fn phase_for(step: MutationStep) -> OperationPhase {
    match step {
        MutationStep::Preparing => OperationPhase::Preparing,
        MutationStep::WaitingLease => OperationPhase::WaitingLease,
        MutationStep::Preflight => OperationPhase::ReadyToDispatch,
        MutationStep::Arming => OperationPhase::Arming,
        MutationStep::InFlight => OperationPhase::InFlight,
        MutationStep::ValidatingChange => OperationPhase::ValidatingChange,
        MutationStep::Committing => OperationPhase::Committing,
        MutationStep::Finalizing => OperationPhase::Finalizing,
        MutationStep::Terminal => OperationPhase::Terminal,
        MutationStep::Reconciling => OperationPhase::Reconciling,
    }
}

/// The effects of a row of tables.
pub fn effects_for(step: MutationStep) -> Vec<EffectSpec> {
    match step {
        MutationStep::Preparing => vec![EffectSpec::PersistIntent],
        MutationStep::WaitingLease => vec![EffectSpec::AcquireResources],
        MutationStep::Preflight => vec![EffectSpec::RevalidateGrant],
        MutationStep::Arming => vec![EffectSpec::PersistDispatchArmed],
        MutationStep::InFlight => vec![EffectSpec::SendMutatingCommand],
        MutationStep::ValidatingChange => vec![EffectSpec::QueryProviderState],
        // Without a reconciliation result, you cannot choose one of two transactions.
        MutationStep::Committing => Vec::new(),
        MutationStep::Finalizing => vec![EffectSpec::ReleaseResources],
        MutationStep::Terminal => vec![EffectSpec::RecordTerminal],
        MutationStep::Reconciling => vec![
            EffectSpec::PersistNeedsReconciliation,
            EffectSpec::QuarantineScope,
        ],
    }
}
