use crate::{
    decision::TransitionError,
    input::{EffectReceipt, EffectSpec},
    transition::{OperationBody, OperationPhase, OperationState, TerminalOutcome},
};

use super::{
    cancel_for_suppression::cancel_for_suppression,
    destroy_held_if_needed::destroy_held_if_needed,
    dispatch_outcome::{dispatch_not_sent, dispatch_unknown},
};

/// Confirmation of the effects of the inspection machine.
pub(super) fn receipt_step(
    state: &OperationState,
    spec: EffectSpec,
    receipt: EffectReceipt,
    next: &mut OperationState,
) -> Result<(OperationPhase, Vec<EffectSpec>), TransitionError> {
    use EffectSpec as E;

    match (state.common.phase, spec, receipt) {
        (phase, E::SuppressResult, EffectReceipt::ResultSuppressed) => Ok((phase, Vec::new())),
        (OperationPhase::WaitingLease, E::AcquireResources, EffectReceipt::LeaseGranted) => Ok((
            OperationPhase::ReadyToDispatch,
            vec![EffectSpec::RevalidateGrant],
        )),
        (OperationPhase::WaitingLease, E::AcquireResources, EffectReceipt::LeaseDenied) => {
            next.common.terminal = Some(TerminalOutcome::Unavailable);
            Ok((OperationPhase::Finalizing, vec![EffectSpec::RecordTerminal]))
        }
        (OperationPhase::ReadyToDispatch, E::RevalidateGrant, EffectReceipt::GrantValid) => {
            Ok((OperationPhase::Running, vec![EffectSpec::SendVerify]))
        }
        (OperationPhase::ReadyToDispatch, E::RevalidateGrant, EffectReceipt::GrantRevoked) => {
            cancel_for_suppression(state, next)
        }
        (OperationPhase::Running, E::SendVerify, EffectReceipt::SendAccepted) => {
            // It's sent, but it doesn't mean that the hardware has responded.
            Ok((OperationPhase::Running, Vec::new()))
        }
        (OperationPhase::Running, E::SendVerify, EffectReceipt::DispatchNotSent) => {
            dispatch_not_sent(next)
        }
        (OperationPhase::Running, E::SendVerify, EffectReceipt::DispatchOutcomeUnknown) => {
            dispatch_unknown(next)
        }
        (
            OperationPhase::EvaluatingEvidence,
            E::VerifyEvidence,
            EffectReceipt::EvidenceAccepted,
        ) => {
            Ok((
                OperationPhase::EvaluatingEvidence,
                vec![EffectSpec::RevalidateOwner],
            ))
        }
        (
            OperationPhase::EvaluatingEvidence,
            E::VerifyEvidence,
            EffectReceipt::EvidenceRejected,
        ) => {
            // The failure of the inspector does not indicate the result of the comparison.
            next.common.terminal = Some(TerminalOutcome::Failed);
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::DestroyEvidence, EffectSpec::ReleaseResources],
            ))
        }
        (OperationPhase::EvaluatingEvidence, E::RevalidateOwner, EffectReceipt::OwnerValid) => {
            next.common.terminal = Some(TerminalOutcome::FactorAccepted);
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::ReleaseResources],
            ))
        }
        (OperationPhase::EvaluatingEvidence, E::RevalidateOwner, EffectReceipt::OwnerRevoked) => {
            // Suppression takes precedence over success (E-01).
            next.common.terminal = Some(TerminalOutcome::Cancelled);
            let mut effects = vec![EffectSpec::SuppressResult, EffectSpec::ReleaseResources];
            destroy_held_if_needed(state, &mut effects);
            Ok((OperationPhase::Finalizing, effects))
        }
        (OperationPhase::Finalizing, E::ReleaseResources, EffectReceipt::CleanupConfirmed) => {
            // The Trustee confirmed the release of this issue; in case of uncertainty, the
            // reference remains with the operation.
            next.common.lease = None;
            if state
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == E::DestroyEvidence)
            {
                // The terminal recording awaits both confirmations: cleaning up and
                // destroying the withheld evidence.
                return Ok((OperationPhase::Finalizing, Vec::new()));
            }
            let effect = if state.common.terminal == Some(TerminalOutcome::FactorAccepted) {
                EffectSpec::RevalidateOwner
            } else {
                EffectSpec::RecordTerminal
            };
            Ok((OperationPhase::Finalizing, vec![effect]))
        }
        (OperationPhase::Finalizing, E::RevalidateOwner, EffectReceipt::OwnerValid)
            if state.common.terminal == Some(TerminalOutcome::FactorAccepted) =>
        {
            Ok((OperationPhase::Finalizing, vec![EffectSpec::DeliverOutcome]))
        }
        (OperationPhase::Finalizing, E::RevalidateOwner, EffectReceipt::OwnerRevoked) => {
            next.common.terminal = Some(TerminalOutcome::Cancelled);
            let mut effects = vec![EffectSpec::SuppressResult];
            if state
                .body
                .verification()
                .is_some_and(|verification| verification.evidence.is_some())
            {
                destroy_held_if_needed(state, &mut effects);
            } else {
                effects.push(EffectSpec::RecordTerminal);
            }
            Ok((OperationPhase::Finalizing, effects))
        }
        (
            OperationPhase::Finalizing,
            E::RequestCancel,
            EffectReceipt::CancelDispatched
            | EffectReceipt::CancelUnsupported
            | EffectReceipt::CancelWorkerStopped,
        ) => Ok((
            OperationPhase::Finalizing,
            vec![EffectSpec::ReleaseResources],
        )),
        (OperationPhase::Finalizing, E::ReleaseResources, EffectReceipt::ResourceUncertain) => {
            // Unsuccessful cleaning does not deliver previously prepared success: the safety
            // of resources has not yet been confirmed.
            next.common.terminal = Some(TerminalOutcome::Unavailable);
            let mut effects = vec![EffectSpec::QuarantineScope];
            destroy_held_if_needed(state, &mut effects);
            Ok((OperationPhase::Finalizing, effects))
        }
        (
            OperationPhase::Finalizing,
            E::RecordTerminal,
            EffectReceipt::VerificationTerminalRecorded,
        ) if matches!(
            state.common.terminal,
            Some(
                TerminalOutcome::NoSample
                    | TerminalOutcome::NotMatched
                    | TerminalOutcome::Cancelled
                    | TerminalOutcome::Unavailable
                    | TerminalOutcome::Failed
            )
        ) || (state.common.terminal == Some(TerminalOutcome::FactorAccepted)
            && state.common.lease.is_none()
            && state.body.verification().is_some_and(|verification| {
                verification.evidence.is_none()
                    && verification.delivery == crate::transition::VerificationDelivery::Sent
            })
            && !state.common.pending_effects.iter().any(|effect| {
                matches!(
                    effect.spec,
                    E::ReleaseResources
                        | E::DeliverOutcome
                        | E::DestroyEvidence
                        | E::RevalidateOwner
                )
            })) =>
        {
            Ok((OperationPhase::Terminal, Vec::new()))
        }
        (
            OperationPhase::Finalizing,
            E::DeliverOutcome,
            EffectReceipt::OwnerDeliveryRecorded | EffectReceipt::OwnerDisconnected,
        ) => {
            if receipt == EffectReceipt::OwnerDisconnected {
                next.common.terminal = Some(TerminalOutcome::Cancelled);
            } else if state.common.terminal == Some(TerminalOutcome::FactorAccepted)
                && let OperationBody::Verification(verification) = &mut next.body
            {
                verification.delivery = crate::transition::VerificationDelivery::Sent;
            }
            let effect = if state
                .body
                .verification()
                .is_some_and(|verification| verification.evidence.is_some())
            {
                EffectSpec::DestroyEvidence
            } else {
                EffectSpec::RecordTerminal
            };
            Ok((OperationPhase::Finalizing, vec![effect]))
        }
        (
            OperationPhase::Finalizing | OperationPhase::Cancelling,
            E::DestroyEvidence,
            EffectReceipt::EvidenceDestroyed,
        ) => {
            if let OperationBody::Verification(verification) = &mut next.body {
                verification.evidence = None;
            }
            let cleanup_pending = state.common.pending_effects.iter().any(|effect| {
                matches!(
                    effect.spec,
                    E::ReleaseResources
                        | E::AcquireResources
                        | E::RequestCancel
                        | E::QuarantineScope
                        | E::DeliverOutcome
                        | E::RevalidateOwner
                        | E::VerifyEvidence
                )
            });
            if state.common.phase == OperationPhase::Finalizing
                && !cleanup_pending
                && matches!(
                    state.common.terminal,
                    Some(
                        TerminalOutcome::Failed
                            | TerminalOutcome::Cancelled
                            | TerminalOutcome::NotMatched
                            | TerminalOutcome::NoSample
                            | TerminalOutcome::FactorAccepted
                    )
                )
            {
                Ok((OperationPhase::Finalizing, vec![EffectSpec::RecordTerminal]))
            } else {
                Ok((state.common.phase, Vec::new()))
            }
        }
        (OperationPhase::Finalizing, E::QuarantineScope, EffectReceipt::ScopeQuarantined) => {
            Ok((OperationPhase::Finalizing, Vec::new()))
        }
        (
            OperationPhase::Cancelling,
            E::RequestCancel,
            EffectReceipt::CancelDispatched
            | EffectReceipt::CancelUnsupported
            | EffectReceipt::CancelWorkerStopped,
        ) => Ok((
            OperationPhase::Finalizing,
            vec![EffectSpec::ReleaseResources],
        )),
        (OperationPhase::Cancelling, E::QuarantineScope, EffectReceipt::ScopeQuarantined) => {
            Ok((OperationPhase::Cancelling, Vec::new()))
        }
        (OperationPhase::Cancelling, E::AcquireResources, EffectReceipt::LeaseGranted) => {
            // Leasing exists, and the operation can only be completed after a confirmed
            // cleaning.
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::ReleaseResources],
            ))
        }
        (OperationPhase::Cancelling, E::AcquireResources, EffectReceipt::LeaseDenied) => {
            // There was no issue; the pending confirmation is filmed at the address.
            Ok((OperationPhase::Finalizing, vec![EffectSpec::RecordTerminal]))
        }
        (OperationPhase::Cancelling, E::ReleaseResources, EffectReceipt::CleanupConfirmed) => {
            let effects = if state
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == E::DestroyEvidence)
            {
                Vec::new()
            } else {
                vec![EffectSpec::RecordTerminal]
            };
            Ok((OperationPhase::Finalizing, effects))
        }
        (OperationPhase::Cancelling, E::ReleaseResources, EffectReceipt::ResourceUncertain) => {
            next.common.terminal = Some(TerminalOutcome::Unavailable);
            let mut effects = vec![EffectSpec::QuarantineScope];
            destroy_held_if_needed(state, &mut effects);
            Ok((OperationPhase::Finalizing, effects))
        }
        _ => Err(TransitionError::ReceiptMismatch),
    }
}
