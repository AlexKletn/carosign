use super::*;
use crate::reconcile::{ExpectedStates, ReconciliationFailure, VerifiedState, evaluate};
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::ids::{EnrollmentId, EvidenceId};

fn id(byte: u8) -> EnrollmentId {
    EnrollmentId::from_bytes([byte; 16]).expect("валид")
}

fn verified(byte: u8) -> VerifiedState {
    VerifiedState {
        enrollment: id(byte),
        generation: EnrollmentGeneration::FIRST,
        provenance: EvidenceId::from_bytes([9u8; 16]).expect("валид"),
    }
}

#[test]
fn trusted_live_result_commits_but_never_resolves_recovery() {
    let mut state = crate::tests_support::initial();
    state.common.phase = OperationPhase::ValidatingChange;
    let result = step(&state, MutationInput::TrustedProviderCompleted).unwrap();
    assert_eq!(result.phase, OperationPhase::Committing);
    assert_eq!(
        result.registry,
        Some(crate::reconcile::RegistryAction::ApplyDesiredState)
    );
    assert_eq!(
        result.effects,
        vec![EffectSpec::CommitDesiredRegistryAndJournal]
    );
    state.common.phase = OperationPhase::Reconciling;
    assert!(step(&state, MutationInput::TrustedProviderCompleted).is_err());
}

#[test]
fn successful_reconciliation_always_goes_through_committing() {
    // B-01: Direct Reconciling → Terminal is prohibited.
    // Only checking the transition of the machine with the finished result: it does not prove
    // that the current inspector is able to give such a result.
    let verdict = ReconciliationVerdict::DesiredStateConfirmed(verified(2));
    let outcome = reconciliation_outcome(verdict).expect("переход есть");
    assert_eq!(outcome.phase, OperationPhase::Committing);
    assert_eq!(
        outcome.registry,
        Some(crate::reconcile::RegistryAction::ApplyDesiredState)
    );
}

#[test]
fn indeterminate_blocks_commit() {
    let verdict = evaluate(
        ExpectedStates {
            pre: id(1),
            desired: id(2),
        },
        None,
        Some(ReconciliationFailure::Unsupported),
    );
    let outcome = reconciliation_outcome(verdict).expect("переход есть");
    assert_eq!(outcome.phase, OperationPhase::Reconciling);
    assert_eq!(outcome.terminal, Some(TerminalOutcome::NeedsReconciliation));
    assert!(outcome.registry.is_none());
}

#[test]
fn metadata_failure_after_arm_goes_to_reconciling() {
    // B-02: Failure to record after permission - reconciliation, lock remains.
    let mut state = crate::tests_support::initial();
    state.common.phase = OperationPhase::Committing;
    let outcome = step(&state, MutationInput::IntentFailed).expect("переход есть");
    assert_eq!(outcome.phase, OperationPhase::Reconciling);
    assert!(outcome.effects.contains(&EffectSpec::QuarantineScope));
}

#[test]
fn unavailable_storage_never_claims_durable_state() {
    // B-02: If recording is not possible, only lock, no promises.
    let state = crate::tests_support::initial();
    let outcome = step(&state, MutationInput::StorageUnavailable).expect("переход есть");
    assert_eq!(outcome.phase, OperationPhase::Reconciling);
    assert_eq!(outcome.effects, vec![EffectSpec::QuarantineScope]);
    assert!(
        !outcome
            .effects
            .contains(&EffectSpec::PersistNeedsReconciliation)
    );
}

#[test]
fn send_confirmed_not_delivered_is_not_a_repeat() {
    let mut state = crate::tests_support::initial();
    state.common.phase = OperationPhase::InFlight;
    let outcome = step(&state, MutationInput::NotSent).expect("переход есть");
    assert_eq!(outcome.phase, OperationPhase::Reconciling);
    assert!(!outcome.effects.contains(&EffectSpec::SendMutatingCommand));
}
