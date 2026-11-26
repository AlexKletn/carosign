use super::*;
use crate::input::EffectSpec;
use crate::transition::{OperationBody, OperationPhase, TransitionIndex};
use crate::{input::PlannedEffect, transition::OperationState};

fn planned(spec: EffectSpec) -> PlannedEffect {
    PlannedEffect::at(spec, TransitionIndex::new(1), 0)
}

fn state_in(phase: OperationPhase) -> OperationState {
    let mut state = crate::tests_support::initial();
    state.common.phase = phase;
    state
}

#[test]
fn mutation_send_outside_in_flight_is_rejected() {
    let mut next = state_in(OperationPhase::ReadyToDispatch);
    let err = check(&next, &[planned(EffectSpec::SendMutatingCommand)]);
    assert!(err.is_err());
    next.common.phase = OperationPhase::InFlight;
    assert!(check(&next, &[planned(EffectSpec::SendMutatingCommand)]).is_ok());
}

#[test]
fn arm_and_send_in_one_transition_is_rejected() {
    let next = state_in(OperationPhase::Arming);
    let err = check(
        &next,
        &[
            planned(EffectSpec::PersistDispatchArmed),
            planned(EffectSpec::SendMutatingCommand),
        ],
    );
    assert!(err.is_err());
}

#[test]
fn terminal_requires_outcome() {
    let next = state_in(OperationPhase::Terminal);
    assert!(check(&next, &[]).is_err());
}

#[test]
fn held_evidence_blocks_terminal_and_terminal_record() {
    let mut state = state_in(OperationPhase::Finalizing);
    state.common.terminal = Some(crate::transition::TerminalOutcome::Cancelled);
    let id = carosign_core::ids::EvidenceId::from_bytes([1; 16]).unwrap();
    let operation = state.common.context.operation_id;
    let OperationBody::Verification(verification) = &mut state.body else {
        unreachable!()
    };
    verification.evidence = Some(carosign_core::evidence::EvidenceRef::new(id, operation));
    assert!(check(&state, &[planned(EffectSpec::RecordTerminal)]).is_err());
    state
        .common
        .pending_effects
        .push(planned(EffectSpec::RecordTerminal));
    assert!(check(&state, &[]).is_err());
    state.common.pending_effects.clear();
    state.common.phase = OperationPhase::Terminal;
    assert!(check(&state, &[]).is_err());
    let OperationBody::Verification(verification) = &mut state.body else {
        unreachable!()
    };
    verification.evidence = None;
    assert!(check(&state, &[]).is_ok());
}
