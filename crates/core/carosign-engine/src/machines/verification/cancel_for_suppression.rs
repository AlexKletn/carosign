use crate::{
    decision::TransitionError,
    input::EffectSpec,
    transition::{OperationPhase, OperationState, TerminalOutcome},
};

use super::destroy_held_if_needed::destroy_held_if_needed;

/// Review, deadline or request for cancellation - in cancellation with suppression of the
/// result.
pub(super) fn cancel_for_suppression(
    state: &OperationState,
    next: &mut OperationState,
) -> Result<(OperationPhase, Vec<EffectSpec>), TransitionError> {
    if state.common.phase == OperationPhase::Finalizing
        && state.common.terminal == Some(TerminalOutcome::FactorAccepted)
        && state.body.verification().is_some_and(|verification| {
            verification.delivery == crate::transition::VerificationDelivery::Sent
        })
    {
        next.common.cancel_requested = true;
        let mut effects = Vec::new();
        destroy_held_if_needed(state, &mut effects);
        return Ok((OperationPhase::Finalizing, effects));
    }
    // We do not turn the uncertainty of the resource into a normal cancellation: the recall
    // does not confirm either the cleaning or the removal of the quarantine.
    next.common.terminal = if state.common.terminal == Some(TerminalOutcome::Unavailable) {
        Some(TerminalOutcome::Unavailable)
    } else {
        Some(TerminalOutcome::Cancelled)
    };
    next.common.cancel_requested = true;
    let mut effects = if matches!(
        state.common.phase,
        OperationPhase::Running | OperationPhase::ReadyToDispatch
    ) {
        vec![EffectSpec::RequestCancel, EffectSpec::SuppressResult]
    } else {
        vec![EffectSpec::SuppressResult]
    };
    if state.common.phase == OperationPhase::EvaluatingEvidence {
        // The provider’s response has already been received, but the leasing still needs to
        // be removed.
        effects.push(EffectSpec::ReleaseResources);
    }
    destroy_held_if_needed(state, &mut effects);
    let phase = if state.common.phase == OperationPhase::Finalizing {
        // Here, cleaning has already been requested or confirmed. A change to Cancelling
        // would have lost the way from destroying material to terminal recording.
        OperationPhase::Finalizing
    } else {
        OperationPhase::Cancelling
    };
    Ok((phase, effects))
}
