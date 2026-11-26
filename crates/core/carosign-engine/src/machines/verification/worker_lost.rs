use crate::{
    decision::TransitionError,
    input::EffectSpec,
    transition::{OperationPhase, OperationState, TerminalOutcome},
};

use super::destroy_held_if_needed::destroy_held_if_needed;

/// The worker is missing: the inspection cannot continue.
///
/// ⚠️ Losing the process means that the hardware has calmed down, and if resources are given
/// away, they remain in uncertainty until confirmed harvesting.
pub(super) fn worker_lost(
    state: &OperationState,
    next: &mut OperationState,
) -> Result<(OperationPhase, Vec<EffectSpec>), TransitionError> {
    // After confirmed delivery, the result can no longer be replaced by another.
    // Delivery of success is allowed only after a confirmed cleaning of the lease; withheld
    // proof still requires destruction.
    if state.common.phase == OperationPhase::Finalizing
        && state.common.terminal == Some(TerminalOutcome::FactorAccepted)
        && state.common.lease.is_none()
        && state.body.verification().is_some_and(|verification| {
            verification.delivery == crate::transition::VerificationDelivery::Sent
        })
    {
        let mut effects = Vec::new();
        destroy_held_if_needed(state, &mut effects);
        return Ok((OperationPhase::Finalizing, effects));
    }
    next.common.terminal = Some(TerminalOutcome::Unavailable);
    let mut effects = vec![EffectSpec::SuppressResult, EffectSpec::QuarantineScope];
    destroy_held_if_needed(state, &mut effects);
    Ok((OperationPhase::Finalizing, effects))
}
