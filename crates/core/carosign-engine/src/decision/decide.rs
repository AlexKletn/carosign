use super::Decision;
use super::TransitionError;
use crate::input::DomainInput;
use crate::input::EffectReceipt;
use crate::input::EffectSpec;
use crate::input::PlannedEffect;
use crate::invariant;
use crate::machines::enrollment;
use crate::machines::verification;
use crate::transition::OperationBody;
use crate::transition::OperationPhase;
use crate::transition::OperationState;
use carosign_core::modality::OperationKind;

/// The only entrance to the transition car.
///
/// ⚠️ Pure function: the same state and the same input give the same answer
/// (E-07).`OperationBody`](crate::transition::OperationBody) (C-03); no sign of the species.
pub fn decide(state: &OperationState, input: DomainInput) -> Result<Decision, TransitionError> {
    invariant::check_scope(state)?;
    invariant::check_reconciliation(state)?;
    invariant::check_phase(state.kind(), state.common.phase)?;
    invariant::check_effect_addresses(state)?;
    if state.common.phase.is_terminal() {
        return Err(TransitionError::AlreadyTerminal);
    }
    if invariant::is_stale(state, &input) {
        return Err(TransitionError::StaleFencing);
    }
    if matches!(
        state.body,
        OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
    ) && matches!(
        &input,
        DomainInput::Receipt {
            effect: PlannedEffect {
                spec: EffectSpec::PersistDispatchArmed,
                ..
            },
            receipt: EffectReceipt::DispatchArmedDurable,
        }
    ) {
        let (Some(_), Some(worker), Some(command)) = (
            state.common.lease,
            state.common.worker,
            state.common.command,
        ) else {
            return Err(TransitionError::ReceiptMismatch);
        };
        if worker.device_epoch != state.common.context.authenticator.device_epoch {
            return Err(TransitionError::StaleFencing);
        }
        if state
            .common
            .transition_index
            .next()
            .map_err(|_| TransitionError::TransitionIndexOverflow)?
            != command.transition_index
        {
            // Refusal does not disprove a stable record: the owner needs a reconciliation.
            return Err(TransitionError::ReceiptMismatch);
        }
    }

    // ⚠️ Confirmation goes to the **issued ** effect: it's removed and only it. Remember the
    // address before the transition - after it. `input` Already eaten by the branches.
    let confirmed = match &input {
        DomainInput::Receipt { effect, .. } => Some(*effect),
        _ => None,
    };
    let mutation_progress = matches!(input, DomainInput::Progress)
        && matches!(
            state.body,
            OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
        )
        && state.common.phase == OperationPhase::InFlight;
    let worker_lost = matches!(input, DomainInput::WorkerLost);
    let storage_unavailable = matches!(input, DomainInput::StorageUnavailable);
    let resource_uncertain = matches!(
        input,
        DomainInput::Receipt {
            effect: PlannedEffect {
                spec: EffectSpec::ReleaseResources,
                ..
            },
            receipt: EffectReceipt::ResourceUncertain,
        }
    );
    let reconciliation = match &input {
        DomainInput::Reconciliation(verdict) if verdict.allows_commit() => Some(*verdict),
        _ => None,
    };

    let mut next = state.clone();
    if matches!(
        state.body,
        OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
    ) && matches!(
        input,
        DomainInput::OwnerRevoked
            | DomainInput::OwnerDetached
            | DomainInput::DeadlineElapsed
            | DomainInput::CancelRequested
    ) {
        next.common.cancel_requested = true;
    }
    let carried = state.common.pending_effects.clone();

    // The choice of the handler is on the body of the operation and nothing else.
    let progress_receipt = matches!(
        &input,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::ProgressPublished,
        } if effect.spec == EffectSpec::PublishProgress
            && state.common.pending_effects.contains(effect)
    );
    let (phase, effects, terminal) = if progress_receipt {
        (state.common.phase, Vec::new(), None)
    } else {
        match &state.body {
            OperationBody::Verification(_) => {
                let (phase, effects) = verification::step(state, &input, &mut next)?;
                (phase, effects, None)
            }
            OperationBody::Enrollment { .. } | OperationBody::Removal { .. } => {
                if let DomainInput::BindMutationDispatch {
                    effect,
                    lease,
                    worker,
                    command,
                } = &input
                {
                    let mut arms = state
                        .common
                        .pending_effects
                        .iter()
                        .filter(|pending| pending.spec == EffectSpec::PersistDispatchArmed);
                    if state.common.phase != OperationPhase::Arming
                        || state.common.cancel_requested
                        || state.common.worker.is_some()
                        || state.common.command.is_some()
                        || state
                            .common
                            .lease
                            .is_some_and(|previous| previous != *lease)
                        || effect.spec != EffectSpec::PersistDispatchArmed
                        || arms.next() != Some(effect)
                        || arms.next().is_some()
                    {
                        return Err(TransitionError::UnexpectedInput {
                            phase: state.common.phase,
                        });
                    }
                    if worker.device_epoch != state.common.context.authenticator.device_epoch {
                        return Err(TransitionError::StaleFencing);
                    }
                    let send_transition = state
                        .common
                        .transition_index
                        .next()
                        .and_then(|index| index.next())
                        .map_err(|_| TransitionError::TransitionIndexOverflow)?;
                    next.common.lease = Some(*lease);
                    next.common.worker = Some(*worker);
                    next.common.command = Some(crate::transition::CommandRef {
                        id: *command,
                        transition_index: send_transition,
                    });
                    (OperationPhase::Arming, Vec::new(), None)
                } else if mutation_progress {
                    (
                        OperationPhase::InFlight,
                        vec![EffectSpec::PublishProgress],
                        None,
                    )
                } else {
                    let mutation_input = mutation_input_of(state, input)?;
                    let outcome = enrollment::step(state, mutation_input)?;
                    if let (Some(action), Some(mutation)) =
                        (outcome.registry, next.body.mutation_mut())
                    {
                        mutation.registry = Some(action);
                        mutation.reconciliation = reconciliation;
                        use crate::transition::LiveMutationConfirmation;
                        mutation.live_confirmation = match mutation_input {
                            enrollment::MutationInput::TrustedProviderCompleted => {
                                Some(LiveMutationConfirmation::TrustedProvider)
                            }
                            enrollment::MutationInput::TrustedEnrollmentRollback => {
                                Some(LiveMutationConfirmation::TrustedEnrollmentRollback)
                            }
                            _ => None,
                        };
                    }
                    // ⚠️ The change machine doesn't really rule the general state, so we copy
                    // what it's called, obviously.
                    if let Some(lease) = outcome.lease_taken {
                        next.common.lease = Some(lease);
                    }
                    if let Some(command) = outcome.command_sent {
                        next.common.command = Some(command);
                    }
                    (outcome.phase, outcome.effects, outcome.terminal)
                }
            }
        }
    };

    next.common.phase = phase;
    if phase == OperationPhase::Reconciling
        && state.common.phase == OperationPhase::Committing
        && let Some(mutation) = next.body.mutation_mut()
    {
        // The previously selected result does not allow repeating the ambiguous fixation. The
        // true phase is first read from the disk.
        mutation.reconciliation = None;
        mutation.live_confirmation = None;
    }
    if let Some(terminal) = terminal {
        next.common.terminal = Some(terminal);
    }
    invariant::check_phase(next.kind(), phase)?;
    invariant::check_reconciliation(&next)?;

    // Transition count increases by exactly 1 for a successful transition.
    next.common.transition_index = state
        .common
        .transition_index
        .next()
        .map_err(|_| TransitionError::TransitionIndexOverflow)?;

    // ⚠️ Effects that the engine only **repeats** (already published, with the same place)
    // are not republished: the place is set by the effect itself, not the step.
    // A reissue would give the new place an old confirmation - and the confirmation would
    // cease to find its effect.
    // ⚠️ The exception is `Progress` and other inputs without confirmation: they return
    // waiting effects to memory (see below).`pending_effects`), and what has already been
    // published is not published a second time.
    let repeats_issued = confirmed.is_none();
    let effects: Vec<EffectSpec> = effects
        .into_iter()
        .filter(|spec| {
            !(repeats_issued
                && state
                    .common
                    .pending_effects
                    .iter()
                    .any(|pending| pending.spec == *spec))
        })
        .collect();

    // The place of each effect in the transition is in order; this is the key on the disk.
    let index = next.common.transition_index;
    let planned: Vec<PlannedEffect> = effects
        .into_iter()
        .enumerate()
        .map(|(position, spec)| {
            PlannedEffect::at(spec, index, u32::try_from(position).unwrap_or(u32::MAX))
        })
        .collect();
    // ⚠️ Confirmed pending are filmed, unconfirmed remain: published is added. `Progress`
    // Once confirmed, I would keep an outdated list, and the next step would have no effect.
    // ⚠️ The transition does not reissue the previous effects: the waiting is the state, not
    // the inference of the transition. The confirmed is removed, the new is added.
    next.common.pending_effects = match confirmed {
        Some(effect) => carried
            .into_iter()
            .filter(|pending| *pending != effect)
            .collect(),
        None => carried,
    };
    // Terminal surveillance by the provider can come without a separate ACK.
    // After the exit from the sending phase, the old effect cannot be performed again.
    match &next.body {
        OperationBody::Verification(_) if next.common.phase != OperationPhase::Running => {
            next.common
                .pending_effects
                .retain(|effect| effect.spec != EffectSpec::SendVerify);
        }
        OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
            if next.common.phase != OperationPhase::InFlight =>
        {
            next.common
                .pending_effects
                .retain(|effect| effect.spec != EffectSpec::SendMutatingCommand);
        }
        _ => {}
    }
    // Negative outcome prohibits further sending and issuing success, including in case of
    // loss of an worker or ambiguous sending. already started execution may respond late;
    // the effects of cleaning are preserved.
    if matches!(next.body, OperationBody::Verification(_))
        && next.common.terminal.is_some()
        && next.common.terminal != Some(crate::transition::TerminalOutcome::FactorAccepted)
    {
        next.common.pending_effects.retain(|effect| {
            !matches!(
                effect.spec,
                EffectSpec::RevalidateGrant
                    | EffectSpec::SendVerify
                    | EffectSpec::VerifyEvidence
                    | EffectSpec::RevalidateOwner
                    | EffectSpec::DeliverOutcome
            )
        });
    }
    // A previously issued completion must not survive a transition in which the proof is
    // still withheld and its revocation is requested.
    if next
        .body
        .verification()
        .is_some_and(|verification| verification.evidence.is_some())
        && (planned
            .iter()
            .any(|effect| effect.spec == EffectSpec::DestroyEvidence)
            || next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::DestroyEvidence))
    {
        next.common
            .pending_effects
            .retain(|effect| effect.spec != EffectSpec::RecordTerminal);
    }
    // The loss of an worker or the unclear outcome of a cleaning operation does not confirm
    // peace of mind: previous requests for cleaning and completion should not circumvent
    // quarantine.
    if (worker_lost || resource_uncertain) && matches!(next.body, OperationBody::Verification(_)) {
        next.common.pending_effects.retain(|effect| {
            !matches!(
                effect.spec,
                EffectSpec::RequestCancel
                    | EffectSpec::ReleaseResources
                    | EffectSpec::RecordTerminal
            )
        });
    }
    // After the transition to reconciliation, the previously issued sending and fixing are
    // not repeated: the result could be performed with a lost confirmation.
    if matches!(
        next.body,
        OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
    ) && next.common.phase == OperationPhase::Reconciling
    {
        next.common.pending_effects.retain(|effect| {
            !matches!(
                effect.spec,
                EffectSpec::PersistDispatchArmed
                    | EffectSpec::SendMutatingCommand
                    | EffectSpec::CommitDesiredRegistryAndJournal
                    | EffectSpec::CommitNoChangeAndJournal
                    | EffectSpec::RecordTerminal
            )
        });
        if worker_lost {
            next.common
                .pending_effects
                .retain(|effect| effect.spec != EffectSpec::ReleaseResources);
        }
    }
    if storage_unavailable
        && matches!(
            next.body,
            OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
        )
    {
        // Do not repeat ambiguous entries and do not continue to issue resources until the
        // stable state is not available for re-reading.
        next.common.pending_effects.retain(|effect| {
            !matches!(
                effect.spec,
                EffectSpec::PersistIntent
                    | EffectSpec::PersistNeedsReconciliation
                    | EffectSpec::AcquireResources
                    | EffectSpec::RevalidateGrant
                    | EffectSpec::ReleaseResources
            )
        });
    }
    if matches!(
        next.body,
        OperationBody::Enrollment { .. } | OperationBody::Removal { .. }
    ) && next.common.terminal == Some(crate::transition::TerminalOutcome::Cancelled)
    {
        next.common.pending_effects.retain(|effect| {
            !matches!(
                effect.spec,
                EffectSpec::RevalidateGrant
                    | EffectSpec::PersistDispatchArmed
                    | EffectSpec::SendMutatingCommand
            )
        });
    }
    next.common.pending_effects.extend(planned.iter().copied());

    invariant::check(&next, &planned)?;

    Ok(Decision {
        next,
        effects: planned,
    })
}

/// The input of the change in the language of the change machine.
///
/// ⚠️ Confirmation from sending an inspection (`SendVerify`) does not fall here: it belongs
/// to the inspection and receives
/// ...`TransitionError::InvalidInputForOperation`](super::TransitionError::InvalidInputForOperation).
fn mutation_input_of(
    state: &OperationState,
    input: DomainInput,
) -> Result<enrollment::MutationInput, TransitionError> {
    let kind = state.kind();
    use enrollment::MutationInput as M;

    Ok(match input {
        DomainInput::Authorized => M::ManageGrantValid,
        DomainInput::OwnerRevoked
        | DomainInput::OwnerDetached
        | DomainInput::DeadlineElapsed
        | DomainInput::CancelRequested => M::GrantRevoked,
        DomainInput::WorkerLost => M::WorkerLost,
        DomainInput::StorageUnavailable => M::StorageUnavailable,
        DomainInput::Reconciliation(verdict) => M::Reconciled(verdict),
        // ⚠️ The echo is checked here: the engine knows the scheduled registration from the
        // state, and does not take the provider at his word.
        DomainInput::ProviderCompleted { echoed } => {
            let planned = match &state.body {
                OperationBody::Enrollment { state, .. } => state.planned_new,
                OperationBody::Removal { request, .. } => Some(request.enrollment),
                _ => None,
            };
            match planned {
                Some(plan) if plan == echoed => M::ProviderCompleted,
                // ⚠️ The echo didn't match - it's not "nothing changed" or "completed": it's
                // a reconciliation.
                _ => M::OutcomeUnknown,
            }
        }
        DomainInput::ProviderCompletedWithoutEcho => {
            // ⚠️ Document 34: Absence of echo is a violation, condition unknown.
            M::OutcomeUnknown
        }
        DomainInput::ProviderNoChange => M::ProviderNoChange,
        DomainInput::TrustedEnrollmentRollback => M::TrustedEnrollmentRollback,
        DomainInput::ProviderMayHaveOccurred => M::ProviderMayHaveOccurred,
        DomainInput::ProviderFailed => M::ProviderFailed,
        DomainInput::Progress => {
            return Err(TransitionError::NotSpecified {
                what: "ход выполнения не меняет фазу изменения",
            });
        }
        DomainInput::VerificationSent
        | DomainInput::BindVerificationDispatch { .. }
        | DomainInput::VerificationNotSent
        | DomainInput::VerificationOutcomeUnknown => {
            // ⚠️ C-03: Send a check to the change automaton - failure without transition and
            // without side effects.
            return Err(TransitionError::InvalidInputForOperation {
                kind,
                belongs_to: OperationKind::Verify,
            });
        }
        DomainInput::BindMutationDispatch { .. } => {
            return Err(TransitionError::NotSpecified {
                what: "привязка обрабатывается до машины изменения",
            });
        }
        DomainInput::Receipt { effect, receipt } => {
            // ⚠️ First **view: there is no confirmation of a change in the car, and there is
            // no place to talk about it (C-03, E-10). The order of checks is important: a
            // refusal in appearance should not be replaced by a refusal in place - these are
            // two different troubles, and they have a different word.
            if effect.spec == EffectSpec::SendVerify {
                return Err(TransitionError::InvalidInputForOperation {
                    kind,
                    belongs_to: OperationKind::Verify,
                });
            }
            // ⚠️ Document 04: “Every successful transition increments index by exactly 1”;
            // document 28:EffectId derives deterministically from (OperationId,
            // transition_index, effect_positionSo the transition effect N carries the place
            // N, and the confirmation must carry the same.
            //
            // ⚠️ Until 11.11.2026, there was no reconciliation here at all, and the one that
            // was put later ("place = current + 1") rejected the second answer of the port:
            // the operation stood on the ground. `WaitingLease` on `Arming`.
            //
            // Confirmation refers only to the actually published effect and to its kind; a
            // future or otherwise occupied address does not prove that the I/O operation has
            // been performed.
            if !state.common.pending_effects.contains(&effect)
                || !mutation_receipt_matches(effect.spec, receipt)
            {
                return Err(TransitionError::ReceiptMismatch);
            }
            let required_action = match effect.spec {
                EffectSpec::CommitDesiredRegistryAndJournal => {
                    Some(crate::reconcile::RegistryAction::ApplyDesiredState)
                }
                EffectSpec::CommitNoChangeAndJournal => {
                    Some(crate::reconcile::RegistryAction::RecordNoChange)
                }
                _ => None,
            };
            if let Some(required) = required_action {
                use crate::reconcile::RegistryAction;
                use crate::transition::LiveMutationConfirmation;
                let mutation = state.mutation().ok_or(TransitionError::ReceiptMismatch)?;
                let confirmation = match mutation.live_confirmation {
                    Some(LiveMutationConfirmation::TrustedProvider) => {
                        Some(RegistryAction::ApplyDesiredState)
                    }
                    Some(LiveMutationConfirmation::TrustedEnrollmentRollback) => {
                        Some(RegistryAction::RecordNoChange)
                    }
                    None => mutation
                        .reconciliation
                        .and_then(|verdict| verdict.registry_action()),
                };
                if mutation.registry != Some(required) || confirmation != Some(required) {
                    return Err(TransitionError::ReceiptMismatch);
                }
            }
            return receipt_to_mutation(receipt);
        }
        _ => {
            return Err(TransitionError::InvalidInputForOperation {
                kind,
                belongs_to: OperationKind::Verify,
            });
        }
    })
}

fn mutation_receipt_matches(effect: EffectSpec, receipt: EffectReceipt) -> bool {
    use EffectReceipt as R;
    use EffectSpec as E;
    matches!(
        (effect, receipt),
        (E::PersistIntent, R::IntentDurable | R::IntentFailed)
            | (E::AcquireResources, R::LeaseGranted | R::LeaseDenied)
            | (E::RevalidateGrant, R::GrantValid | R::GrantRevoked)
            | (
                E::PersistDispatchArmed,
                R::DispatchArmedDurable | R::DispatchArmedFailed
            )
            | (
                E::SendMutatingCommand,
                R::MutationAccepted | R::MutationNotSent | R::MutationOutcomeUnknown
            )
            | (E::QueryProviderState, R::TrustedProviderCompleted)
            | (
                E::CommitDesiredRegistryAndJournal,
                R::MetadataCommitted | R::MetadataFailed
            )
            | (
                E::CommitNoChangeAndJournal,
                R::MetadataNoChange | R::MetadataFailed
            )
            | (
                E::ReleaseResources,
                R::CleanupConfirmed | R::ResourceUncertain
            )
    )
}

/// Confirmation of entry of the change machine.
fn receipt_to_mutation(
    receipt: EffectReceipt,
) -> Result<enrollment::MutationInput, TransitionError> {
    use enrollment::MutationInput as M;
    Ok(match receipt {
        EffectReceipt::IntentDurable => M::IntentDurable,
        EffectReceipt::IntentFailed => M::IntentFailed,
        EffectReceipt::LeaseGranted => M::LeaseGranted,
        EffectReceipt::LeaseDenied => M::LeaseDenied,
        EffectReceipt::GrantValid => M::GrantValid,
        EffectReceipt::GrantRevoked => M::GrantRevoked,
        EffectReceipt::DispatchArmedDurable => M::DispatchArmedDurable,
        EffectReceipt::DispatchArmedFailed => M::DispatchArmedFailed,
        EffectReceipt::MutationAccepted => M::DispatchAccepted,
        EffectReceipt::MutationNotSent => M::NotSent,
        EffectReceipt::MutationOutcomeUnknown => M::OutcomeUnknown,
        EffectReceipt::TrustedProviderCompleted => M::TrustedProviderCompleted,
        EffectReceipt::MetadataCommitted | EffectReceipt::MetadataNoChange => M::IntentDurable,
        EffectReceipt::MetadataFailed => M::IntentFailed,
        EffectReceipt::CleanupConfirmed => M::IntentDurable,
        EffectReceipt::ResourceUncertain => M::OutcomeUnknown,
        _ => return Err(TransitionError::ReceiptMismatch),
    })
}
