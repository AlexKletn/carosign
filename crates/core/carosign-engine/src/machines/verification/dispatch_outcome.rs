use crate::{
    decision::TransitionError,
    input::EffectSpec,
    transition::{OperationPhase, OperationState, TerminalOutcome},
};

pub(super) fn dispatch_not_sent(
    next: &mut OperationState,
) -> Result<(OperationPhase, Vec<EffectSpec>), TransitionError> {
    next.common.terminal = Some(TerminalOutcome::Failed);
    next.common.failure =
        Some(carosign_core::operation::verification::VerificationFailure::DispatchNotSent);
    Ok((
        OperationPhase::Finalizing,
        vec![EffectSpec::ReleaseResources],
    ))
}

pub(super) fn dispatch_unknown(
    next: &mut OperationState,
) -> Result<(OperationPhase, Vec<EffectSpec>), TransitionError> {
    next.common.terminal = Some(TerminalOutcome::Failed);
    next.common.failure =
        Some(carosign_core::operation::verification::VerificationFailure::DispatchOutcomeUnknown);
    Ok((
        OperationPhase::Cancelling,
        vec![
            EffectSpec::RequestCancel,
            EffectSpec::SuppressResult,
            EffectSpec::QuarantineScope,
        ],
    ))
}
