use super::*;
use crate::transition::OperationPhase;

#[test]
fn cancelling_alone_does_not_prove_no_mutation_was_armed() {
    assert_eq!(
        action_for(OperationPhase::Cancelling),
        RecoveryAction::WaitForDecision
    );
    assert_eq!(
        action_for(OperationPhase::Arming),
        RecoveryAction::NeedsReconciliation
    );
    assert_eq!(
        action_for(OperationPhase::InFlight),
        RecoveryAction::NeedsReconciliation
    );
}
