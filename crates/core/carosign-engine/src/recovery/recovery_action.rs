use crate::transition::OperationPhase;

/// What is expected after the reboot for the operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecoveryAction {
    /// It didn't start: it's safe to refuse.
    Refuse,
    /// Prepared, but not allowed: nothing was confirmed.
    ///
    /// ⚠️ `NoChange` This is only allowed because the permission is not written to the disk;
    /// `DispatchArmed` That would be a lie.
    SafeNoChange,
    /// Allowed, but the outcome is unknown: reconciliation.
    NeedsReconciliation,
    /// Waiting for a decision - the phase does not give a clear answer.
    WaitForDecision,
}

/// What to do with the operation in its phase.
pub fn action_for(phase: OperationPhase) -> RecoveryAction {
    match phase {
        OperationPhase::New | OperationPhase::Preparing => RecoveryAction::Refuse,
        OperationPhase::WaitingLease | OperationPhase::ReadyToDispatch => {
            RecoveryAction::SafeNoChange
        }
        OperationPhase::Arming
        | OperationPhase::InFlight
        | OperationPhase::ValidatingChange
        | OperationPhase::Committing
        | OperationPhase::Reconciling => RecoveryAction::NeedsReconciliation,
        OperationPhase::Running | OperationPhase::EvaluatingEvidence => {
            // Verification after reboot does not continue (document 05).
            RecoveryAction::Refuse
        }
        // The overall phase of cancellation does not indicate a lack of sustainability.
        // DispatchArmedIt's based on the log, not the last phase of the operation.
        OperationPhase::Cancelling => RecoveryAction::WaitForDecision,
        OperationPhase::Finalizing => RecoveryAction::WaitForDecision,
        OperationPhase::Terminal => RecoveryAction::WaitForDecision,
    }
}
