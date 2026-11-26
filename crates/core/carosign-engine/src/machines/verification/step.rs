use crate::{
    decision::TransitionError,
    input::{DomainInput, EffectSpec},
    transition::{OperationBody, OperationPhase, OperationState, TerminalOutcome},
};

use super::{
    cancel_for_suppression::cancel_for_suppression,
    dispatch_outcome::{dispatch_not_sent, dispatch_unknown},
    receipt_step::receipt_step,
    worker_lost::worker_lost,
};

/// One Step Testing Machine: New Phase and Required Effects
///
/// ⚠️ The input for which the document 04 of the table does not give is an
/// error.`TransitionError::NotSpecified`], not "missing."
pub(crate) fn step(
    state: &OperationState,
    input: &DomainInput,
    next: &mut OperationState,
) -> Result<(OperationPhase, Vec<EffectSpec>), TransitionError> {
    match input {
        // The owner is withdrawn or the deadline has expired - suppression of the result.
        DomainInput::OwnerRevoked | DomainInput::DeadlineElapsed | DomainInput::OwnerDetached => {
            cancel_for_suppression(state, next)
        }
        DomainInput::CancelRequested => cancel_for_suppression(state, next),
        DomainInput::WorkerLost => worker_lost(state, next),
        DomainInput::DeviceEpochChanged(_) => Ok((state.common.phase, Vec::new())),
        DomainInput::Progress if state.common.phase == OperationPhase::Running => {
            Ok((OperationPhase::Running, vec![EffectSpec::PublishProgress]))
        }
        DomainInput::Progress => Err(unexpected(state)),

        DomainInput::Authorized => match state.common.phase {
            OperationPhase::New => Ok((
                OperationPhase::WaitingLease,
                vec![EffectSpec::AcquireResources],
            )),
            _ => Err(unexpected(state)),
        },

        DomainInput::BindVerificationDispatch {
            effect,
            lease,
            worker,
            command,
        } => {
            if state.common.phase != OperationPhase::Running
                || state.common.cancel_requested
                || state.common.worker.is_some()
                || state.common.command.is_some()
                || state
                    .common
                    .lease
                    .is_some_and(|previous| previous != *lease)
                || effect.spec != EffectSpec::SendVerify
                || !state.common.pending_effects.contains(effect)
            {
                return Err(unexpected(state));
            }
            if worker.device_epoch != state.common.context.authenticator.device_epoch {
                return Err(TransitionError::StaleFencing);
            }
            next.common.worker = Some(*worker);
            next.common.lease = Some(*lease);
            next.common.command = Some(crate::transition::CommandRef {
                id: *command,
                transition_index: effect.transition,
            });
            Ok((OperationPhase::Running, Vec::new()))
        }

        DomainInput::BindMutationDispatch { .. } => {
            Err(TransitionError::InvalidInputForOperation {
                kind: carosign_core::modality::OperationKind::Verify,
                belongs_to: carosign_core::modality::OperationKind::Enroll,
            })
        }

        DomainInput::Receipt { effect, receipt } => {
            if effect.spec == EffectSpec::SendMutatingCommand || input.is_mutation_send_receipt() {
                return Err(TransitionError::InvalidInputForOperation {
                    kind: carosign_core::modality::OperationKind::Verify,
                    belongs_to: carosign_core::modality::OperationKind::Enroll,
                });
            }
            // ⚠️ The place of confirmation is checked **here**, at the machine itself: in the
            // branch of the change, the check is in `mutation_input_of`And there was no
            // verification at all in the verification branch -- confirmation from someone
            // else's place was accepted, and the addressing error remained invisible.
            // We check with the place of the published effect: neither the performer nor the
            // service has any other sources of space.
            if !state.common.pending_effects.contains(effect) {
                return Err(TransitionError::ReceiptMismatch);
            }
            receipt_step(state, effect.spec, *receipt, next)
        }

        DomainInput::VerificationSent => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            // It was sent, but it doesn't mean that the hardware answered.
            Ok((OperationPhase::Running, Vec::new()))
        }
        DomainInput::VerificationNotSent => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            dispatch_not_sent(next)
        }
        DomainInput::VerificationOutcomeUnknown => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            dispatch_unknown(next)
        }

        DomainInput::ProviderMatched(evidence) => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            // ⚠️ The reference to the proof is stored in the body of the **check*: change
            // does not happen by construction.
            if let Some(verification) = next.body.verification() {
                let _ = verification;
            }
            if let Some(OperationBody::Verification(verification)) = Some(&mut next.body) {
                verification.evidence = Some(carosign_core::evidence::EvidenceRef::new(
                    *evidence,
                    next.common.context.operation_id,
                ));
            }
            next.common.terminal = None;
            Ok((
                OperationPhase::EvaluatingEvidence,
                vec![EffectSpec::VerifyEvidence],
            ))
        }

        DomainInput::ProviderNotMatched => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            next.common.terminal = Some(TerminalOutcome::NotMatched);
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::ReleaseResources],
            ))
        }

        DomainInput::ProviderNoUsableSample => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            next.common.terminal = Some(TerminalOutcome::NoSample);
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::ReleaseResources],
            ))
        }

        DomainInput::ProviderCancelled => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            next.common.terminal = Some(TerminalOutcome::Cancelled);
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::ReleaseResources],
            ))
        }

        DomainInput::ProviderFailed => {
            if state.common.phase != OperationPhase::Running {
                return Err(unexpected(state));
            }
            next.common.terminal = Some(TerminalOutcome::Failed);
            Ok((
                OperationPhase::Finalizing,
                vec![EffectSpec::ReleaseResources],
            ))
        }

        // Changing inputs and reconciliations are a change machine, not a check.
        DomainInput::ProviderCompleted { .. }
        | DomainInput::ProviderCompletedWithoutEcho
        | DomainInput::ProviderNoChange
        | DomainInput::TrustedEnrollmentRollback
        | DomainInput::ProviderMayHaveOccurred
        | DomainInput::Reconciliation(_)
        | DomainInput::StorageUnavailable => Err(TransitionError::NotSpecified {
            what: "вход изменения в машине проверки",
        }),

        DomainInput::_Operation(_) => Err(TransitionError::NotSpecified {
            what: "служебный вход без смысла",
        }),
    }
}

fn unexpected(state: &OperationState) -> TransitionError {
    TransitionError::UnexpectedInput {
        phase: state.common.phase,
    }
}
