//! Machine change through `decide`: decisions B-01 and B-02 on through-paths.
//!
//! ⚠️ It is not the table (it is in the machine module) that is checked here, but the
//! **link**: input routing, output invariants, and the fact that no path cancels any
//! solution.

use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentLabel};
use carosign_core::ids::{EnrollmentId, EvidenceId, MutationId};
use carosign_core::operation::mutation::EnrollRequest;
use carosign_engine::reconcile::{
    ReconciliationFailure, ReconciliationVerdict, RegistryAction, VerifiedState,
};
use carosign_engine::tests_support::initial;
use carosign_engine::{
    DomainInput, EffectReceipt, EffectSpec, OperationPhase, OperationState, PlannedEffect,
    TerminalOutcome, TransitionError, decide,
};

/// Registration status (C-03: Type - body variant, not feature)
fn mutation_state() -> OperationState {
    let context = initial().common.context.clone();
    OperationState::enrollment(
        context,
        EnrollRequest {
            subject: initial().common.context.subject,
            authenticator: initial().common.context.authenticator.id,
            target: EnrollmentLabel::new("проба").expect("метка допустима"),
            replace: None,
        },
        MutationId::from_bytes([7u8; 16]).expect("валид"),
    )
}

#[test]
fn trusted_live_confirmation_allows_only_scoped_durable_commit() {
    let mut state = mutation_state();
    state.common.phase = OperationPhase::ValidatingChange;
    state.common.transition_index = state.common.transition_index.next().unwrap();
    let query = PlannedEffect::at(
        EffectSpec::QueryProviderState,
        state.common.transition_index,
        0,
    );
    state.common.pending_effects.push(query);
    let decision = decide(
        &state,
        DomainInput::Receipt {
            effect: query,
            receipt: EffectReceipt::TrustedProviderCompleted,
        },
    )
    .unwrap();
    assert_eq!(decision.next.common.phase, OperationPhase::Committing);
    let commit = *decision
        .effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::CommitDesiredRegistryAndJournal)
        .unwrap();
    let completed = decide(
        &decision.next,
        DomainInput::Receipt {
            effect: commit,
            receipt: EffectReceipt::MetadataCommitted,
        },
    )
    .unwrap();
    assert_eq!(completed.next.common.phase, OperationPhase::Finalizing);
    let unknown = decide(&decision.next, DomainInput::StorageUnavailable).unwrap();
    assert_eq!(unknown.next.common.phase, OperationPhase::Reconciling);
    assert!(
        decide(
            &unknown.next,
            DomainInput::Receipt {
                effect: query,
                receipt: EffectReceipt::TrustedProviderCompleted,
            }
        )
        .is_err()
    );
}

#[test]
fn ambiguous_arm_failure_quarantines_without_dispatch_or_cleanup() {
    let mut state = mutation_state();
    state.common.phase = OperationPhase::Arming;
    state.common.transition_index = state.common.transition_index.next().unwrap();
    let arm = PlannedEffect::at(
        EffectSpec::PersistDispatchArmed,
        state.common.transition_index,
        0,
    );
    state.common.pending_effects.push(arm);
    let result = decide(
        &state,
        DomainInput::Receipt {
            effect: arm,
            receipt: EffectReceipt::DispatchArmedFailed,
        },
    )
    .unwrap();
    assert_eq!(result.next.common.phase, OperationPhase::Reconciling);
    assert!(
        result
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
    assert!(!result.effects.iter().any(|effect| matches!(
        effect.spec,
        EffectSpec::SendMutatingCommand | EffectSpec::ReleaseResources | EffectSpec::RecordTerminal
    )));
    assert!(result.next.common.terminal.is_none());
}

#[test]
fn final_cleanup_preserves_the_actual_mutation_outcome() {
    use carosign_engine::OperationBody;
    use carosign_engine::machines::enrollment::{MutationInput, step};

    for (registry, prior, expected) in [
        (
            Some(RegistryAction::ApplyDesiredState),
            None,
            TerminalOutcome::MutationCommitted,
        ),
        (
            Some(RegistryAction::RecordNoChange),
            None,
            TerminalOutcome::NoChange,
        ),
        (
            None,
            Some(TerminalOutcome::Cancelled),
            TerminalOutcome::Cancelled,
        ),
        (None, Some(TerminalOutcome::Failed), TerminalOutcome::Failed),
    ] {
        let mut state = mutation_state();
        state.common.phase = OperationPhase::Finalizing;
        state.common.terminal = prior;
        let OperationBody::Enrollment {
            state: mutation, ..
        } = &mut state.body
        else {
            unreachable!()
        };
        mutation.registry = registry;
        let result = step(&state, MutationInput::IntentDurable).unwrap();
        assert_eq!(result.phase, OperationPhase::Terminal);
        assert_eq!(result.effects, vec![EffectSpec::RecordTerminal]);
        assert_eq!(result.terminal, Some(expected));
    }
    let mut state = mutation_state();
    state.common.phase = OperationPhase::Finalizing;
    assert!(step(&state, MutationInput::IntentDurable).is_err());
}

#[test]
fn mutation_finishes_only_after_correlated_cleanup_confirmation() {
    let mut state = mutation_state();
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(TerminalOutcome::Cancelled);
    state.common.transition_index = state.common.transition_index.next().unwrap();
    let release = PlannedEffect::at(
        EffectSpec::ReleaseResources,
        state.common.transition_index,
        0,
    );
    state.common.pending_effects.push(release);

    assert!(matches!(
        decide(
            &state,
            DomainInput::Receipt {
                effect: release,
                receipt: EffectReceipt::MetadataDurable,
            }
        ),
        Err(TransitionError::ReceiptMismatch)
    ));
    let done = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::CleanupConfirmed,
        },
    )
    .unwrap();
    assert_eq!(done.next.common.phase, OperationPhase::Terminal);
    assert_eq!(done.next.common.terminal, Some(TerminalOutcome::Cancelled));
    assert!(
        done.effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RecordTerminal)
    );
    assert!(matches!(
        decide(
            &done.next,
            DomainInput::Receipt {
                effect: release,
                receipt: EffectReceipt::CleanupConfirmed,
            }
        ),
        Err(TransitionError::AlreadyTerminal)
    ));

    let uncertain = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::ResourceUncertain,
        },
    )
    .unwrap();
    assert_eq!(uncertain.next.common.phase, OperationPhase::Reconciling);
    assert!(
        uncertain
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
}

#[test]
fn worker_loss_after_metadata_commit_quarantines_without_releasing_resources() {
    use carosign_engine::OperationBody;
    let mut state = mutation_state();
    state.common.phase = OperationPhase::Finalizing;
    state.common.transition_index = state.common.transition_index.next().unwrap();
    let OperationBody::Enrollment {
        state: mutation, ..
    } = &mut state.body
    else {
        unreachable!()
    };
    mutation.registry = Some(RegistryAction::ApplyDesiredState);
    let release = PlannedEffect::at(
        EffectSpec::ReleaseResources,
        state.common.transition_index,
        0,
    );
    state.common.pending_effects.push(release);

    let decision = decide(&state, DomainInput::WorkerLost).unwrap();
    assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
    assert!(
        !decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::PersistNeedsReconciliation)
    );
    assert!(!decision.next.common.pending_effects.contains(&release));
    assert!(
        !decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::ReleaseResources)
    );
    assert!(
        decision
            .next
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RecoverMetadataCommit)
    );
    for input in [DomainInput::WorkerLost, DomainInput::OwnerRevoked] {
        let repeated = decide(&decision.next, input).unwrap();
        assert!(
            repeated
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::RecoverMetadataCommit)
        );
        assert!(
            !repeated
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::PersistNeedsReconciliation)
        );
    }
}

#[test]
fn revocation_after_metadata_commit_keeps_outcome_and_pending_cleanup() {
    for action in [
        RegistryAction::ApplyDesiredState,
        RegistryAction::RecordNoChange,
    ] {
        for input in [
            DomainInput::OwnerRevoked,
            DomainInput::OwnerDetached,
            DomainInput::DeadlineElapsed,
            DomainInput::CancelRequested,
        ] {
            let mut state = mutation_state();
            state.common.phase = OperationPhase::Finalizing;
            state.common.transition_index = state.common.transition_index.next().unwrap();
            state.body.mutation_mut().unwrap().registry = Some(action);
            let release = PlannedEffect::at(
                EffectSpec::ReleaseResources,
                state.common.transition_index,
                0,
            );
            state.common.pending_effects.push(release);
            let revoked = decide(&state, input).unwrap();
            assert_eq!(revoked.next.common.phase, OperationPhase::Finalizing);
            assert_eq!(revoked.next.mutation().unwrap().registry, Some(action));
            assert!(revoked.next.common.cancel_requested);
            assert!(revoked.next.common.terminal.is_none());
            assert!(revoked.next.common.pending_effects.contains(&release));
            assert!(revoked.effects.is_empty());
            let finished = decide(
                &revoked.next,
                DomainInput::Receipt {
                    effect: release,
                    receipt: EffectReceipt::CleanupConfirmed,
                },
            )
            .unwrap();
            assert_eq!(
                finished.next.common.terminal,
                Some(match action {
                    RegistryAction::ApplyDesiredState => TerminalOutcome::MutationCommitted,
                    RegistryAction::RecordNoChange => TerminalOutcome::NoChange,
                })
            );
        }
    }
}

#[test]
fn uncertain_cleanup_after_metadata_commit_preserves_durable_phase() {
    use carosign_engine::OperationBody;
    for action in [
        RegistryAction::ApplyDesiredState,
        RegistryAction::RecordNoChange,
    ] {
        let mut state = mutation_state();
        state.common.phase = OperationPhase::Finalizing;
        state.common.transition_index = state.common.transition_index.next().unwrap();
        let OperationBody::Enrollment {
            state: mutation, ..
        } = &mut state.body
        else {
            unreachable!()
        };
        mutation.registry = Some(action);
        let release = PlannedEffect::at(
            EffectSpec::ReleaseResources,
            state.common.transition_index,
            0,
        );
        state.common.pending_effects.push(release);
        let decision = decide(
            &state,
            DomainInput::Receipt {
                effect: release,
                receipt: EffectReceipt::ResourceUncertain,
            },
        )
        .unwrap();
        assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
        assert_eq!(
            decision
                .effects
                .iter()
                .map(|effect| effect.spec)
                .collect::<Vec<_>>(),
            vec![
                EffectSpec::QuarantineScope,
                EffectSpec::RecoverMetadataCommit
            ]
        );
        assert_eq!(decision.next.mutation().unwrap().registry, Some(action));
    }
}

#[test]
fn mutation_scope_mismatch_is_rejected_before_any_transition() {
    use carosign_core::ids::{AuthenticatorId, AuthorityId};
    use carosign_engine::OperationBody;

    let original = mutation_state();
    for mismatch in 0..3 {
        let mut state = original.clone();
        let OperationBody::Enrollment { request, .. } = &mut state.body else {
            unreachable!()
        };
        match mismatch {
            0 => request.subject.authority = AuthorityId::from_bytes([99; 16]).unwrap(),
            1 => request.authenticator = AuthenticatorId::from_bytes([99; 16]).unwrap(),
            _ => {
                request.replace = Some(carosign_core::enrollment::EnrollmentRef {
                    id: EnrollmentId::from_bytes([8; 16]).unwrap(),
                    authenticator: AuthenticatorId::from_bytes([99; 16]).unwrap(),
                    generation: EnrollmentGeneration::FIRST,
                })
            }
        }
        let snapshot = state.clone();
        assert!(decide(&state, DomainInput::Authorized).is_err());
        assert_eq!(state, snapshot);
        assert!(state.common.pending_effects.is_empty());
    }
}

#[test]
fn replacement_requires_a_distinct_new_id_in_the_same_authenticator() {
    use carosign_core::enrollment::EnrollmentRef;
    use carosign_core::ids::AuthenticatorId;
    use carosign_engine::OperationBody;

    let mut original = mutation_state();
    let auth = original.common.context.authenticator.id;
    let old = EnrollmentRef {
        id: EnrollmentId::from_bytes([8; 16]).unwrap(),
        authenticator: auth,
        generation: EnrollmentGeneration::FIRST,
    };
    let OperationBody::Enrollment { request, state } = &mut original.body else {
        unreachable!()
    };
    request.replace = Some(old);
    state.planned_new = Some(EnrollmentRef {
        id: EnrollmentId::from_bytes([9; 16]).unwrap(),
        ..old
    });
    assert!(decide(&original, DomainInput::Authorized).is_ok());
    for invalid in [
        old,
        EnrollmentRef {
            id: EnrollmentId::from_bytes([9; 16]).unwrap(),
            authenticator: AuthenticatorId::from_bytes([99; 16]).unwrap(),
            ..old
        },
    ] {
        let mut candidate = original.clone();
        candidate.body.mutation_mut().unwrap().planned_new = Some(invalid);
        assert!(decide(&candidate, DomainInput::Authorized).is_err());
        assert_eq!(candidate.common.transition_index.value(), 0);
    }
}

#[test]
fn removal_rejects_foreign_scope_and_planned_new_registration() {
    use carosign_core::enrollment::EnrollmentRef;
    use carosign_core::ids::{AuthenticatorId, AuthorityId};
    use carosign_core::operation::mutation::RemoveRequest;
    use carosign_engine::OperationBody;

    let context = initial().common.context;
    let target = EnrollmentRef {
        id: EnrollmentId::from_bytes([8; 16]).unwrap(),
        authenticator: context.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let original = OperationState::removal(
        context.clone(),
        RemoveRequest {
            subject: context.subject,
            enrollment: target,
        },
        MutationId::from_bytes([7; 16]).unwrap(),
    );
    assert!(decide(&original, DomainInput::Authorized).is_ok());
    for mismatch in 0..3 {
        let mut candidate = original.clone();
        let OperationBody::Removal { request, state } = &mut candidate.body else {
            unreachable!()
        };
        match mismatch {
            0 => request.subject.authority = AuthorityId::from_bytes([99; 16]).unwrap(),
            1 => request.enrollment.authenticator = AuthenticatorId::from_bytes([99; 16]).unwrap(),
            _ => state.planned_new = Some(target),
        }
        assert!(decide(&candidate, DomainInput::Authorized).is_err());
        assert_eq!(candidate.common.transition_index.value(), 0);
        assert!(candidate.common.pending_effects.is_empty());
    }
}

#[test]
fn removal_accepts_only_echo_of_exact_removed_generation() {
    use carosign_core::enrollment::EnrollmentRef;
    use carosign_core::operation::mutation::RemoveRequest;
    let context = initial().common.context;
    let target = EnrollmentRef {
        id: EnrollmentId::from_bytes([8; 16]).unwrap(),
        authenticator: context.authenticator.id,
        generation: EnrollmentGeneration::FIRST,
    };
    let mut state = OperationState::removal(
        context.clone(),
        RemoveRequest {
            subject: context.subject,
            enrollment: target,
        },
        MutationId::from_bytes([7; 16]).unwrap(),
    );
    state.common.phase = OperationPhase::InFlight;
    let accepted = decide(&state, DomainInput::ProviderCompleted { echoed: target }).unwrap();
    assert_eq!(accepted.next.common.phase, OperationPhase::ValidatingChange);
    let foreign = EnrollmentRef {
        generation: EnrollmentGeneration::new(2).unwrap(),
        ..target
    };
    let rejected = decide(&state, DomainInput::ProviderCompleted { echoed: foreign }).unwrap();
    assert_eq!(rejected.next.common.phase, OperationPhase::Reconciling);
}

fn verified(byte: u8) -> VerifiedState {
    VerifiedState {
        enrollment: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
        generation: EnrollmentGeneration::FIRST,
        provenance: EvidenceId::from_bytes([9u8; 16]).expect("валид"),
    }
}

/// Keeps the state by confirmation, taking place at the **published ** effect.
///
/// ⚠️ So this is what the performer does: you don't guess the place, you take it from the
/// effect that the engine just made, and you guess it, you don't check the addressing at all.
fn advance(mut state: OperationState, steps: &[(EffectSpec, EffectReceipt)]) -> OperationState {
    // ⚠️ Admission, first transition, then confirmation, same as service, first.
    // `Authorized`, then the port's answers.
    state = decide(&state, DomainInput::Authorized)
        .expect("допуск")
        .next;
    for &(spec, receipt) in steps {
        if receipt == EffectReceipt::DispatchArmedDurable {
            state = bind_dispatch(state);
        }
        state = decide(&state, receipt_of(&state, spec, receipt))
            .unwrap_or_else(|error| panic!("{spec:?}: {error}"))
            .next;
    }
    state
}

fn bind_dispatch(state: OperationState) -> OperationState {
    use carosign_core::ids::{CommandId, LeaseId, WorkerInstanceId};
    let effect = state
        .common
        .pending_effects
        .iter()
        .copied()
        .find(|effect| effect.spec == EffectSpec::PersistDispatchArmed)
        .unwrap();
    decide(
        &state,
        DomainInput::BindMutationDispatch {
            effect,
            lease: LeaseId::from_bytes([11; 16]).unwrap(),
            worker: carosign_engine::WorkerRef {
                instance: WorkerInstanceId::from_bytes([12; 16]).unwrap(),
                device_epoch: state.common.context.authenticator.device_epoch,
            },
            command: CommandId::from_bytes([13; 16]).unwrap(),
        },
    )
    .unwrap()
    .next
}

fn receipt_of(state: &OperationState, spec: EffectSpec, receipt: EffectReceipt) -> DomainInput {
    let effect = state
        .common
        .pending_effects
        .iter()
        .copied()
        .find(|pending| pending.spec == spec)
        .unwrap_or_else(|| panic!("эффект {spec:?} ещё не издан"));
    DomainInput::Receipt { effect, receipt }
}

/// Passing the path of registration before `InFlight`.
fn armed() -> OperationState {
    let state = advance(
        mutation_state(),
        &[
            (EffectSpec::PersistIntent, EffectReceipt::IntentDurable),
            (EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
            (EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
            (
                EffectSpec::PersistDispatchArmed,
                EffectReceipt::DispatchArmedDurable,
            ),
        ],
    );
    assert_eq!(state.common.phase, OperationPhase::InFlight);
    state
}

#[test]
fn accepted_mutation_ack_only_confirms_the_correlated_send() {
    let state = armed();
    let send = state
        .common
        .pending_effects
        .iter()
        .copied()
        .find(|effect| effect.spec == EffectSpec::SendMutatingCommand)
        .unwrap();
    let ack = DomainInput::Receipt {
        effect: send,
        receipt: EffectReceipt::MutationAccepted,
    };
    let accepted = decide(&state, ack.clone()).unwrap();
    assert_eq!(accepted.next.common.phase, OperationPhase::InFlight);
    assert!(accepted.next.common.terminal.is_none());
    assert!(!accepted.next.common.pending_effects.contains(&send));
    assert!(accepted.effects.is_empty());
    assert!(decide(&accepted.next, ack).is_err());
    let wrong = DomainInput::Receipt {
        effect: send,
        receipt: EffectReceipt::SendAccepted,
    };
    assert!(decide(&state, wrong).is_err());
    let verify = initial();
    assert!(matches!(
        decide(
            &verify,
            DomainInput::Receipt {
                effect: send,
                receipt: EffectReceipt::MutationAccepted
            }
        ),
        Err(TransitionError::InvalidInputForOperation { .. })
    ));
}

#[test]
fn enrollment_path_arms_before_any_send() {
    let mut state = decide(&mutation_state(), DomainInput::Authorized)
        .expect("переход допустим")
        .next;
    let mut phases = vec![state.common.phase];
    for (spec, receipt) in [
        (EffectSpec::PersistIntent, EffectReceipt::IntentDurable),
        (EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        (EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
    ] {
        let decision = decide(&state, receipt_of(&state, spec, receipt)).expect("переход допустим");
        for effect in &decision.effects {
            // There is never a change-of-sending permission before.
            assert_ne!(effect.spec, EffectSpec::SendMutatingCommand);
        }
        state = decision.next;
        phases.push(state.common.phase);
    }
    assert_eq!(
        phases,
        vec![
            OperationPhase::Preparing,
            OperationPhase::WaitingLease,
            OperationPhase::ReadyToDispatch,
            OperationPhase::Arming,
        ]
    );
    let state = bind_dispatch(state);
    let decision = decide(
        &state,
        receipt_of(
            &state,
            EffectSpec::PersistDispatchArmed,
            EffectReceipt::DispatchArmedDurable,
        ),
    )
    .expect("переход допустим");
    assert_eq!(decision.next.common.phase, OperationPhase::InFlight);
    assert_eq!(decision.effects[0].spec, EffectSpec::SendMutatingCommand);
}

#[test]
fn progress_after_dispatch_does_not_confirm_send() {
    let state = armed();
    let decision = decide(&state, DomainInput::Progress).unwrap();
    assert_eq!(decision.next.common.phase, OperationPhase::InFlight);
    assert_eq!(
        decision.next.common.transition_index,
        state.common.transition_index.next().unwrap()
    );
    assert_eq!(decision.effects.len(), 1);
    assert_eq!(decision.effects[0].spec, EffectSpec::PublishProgress);
    assert!(
        decision
            .next
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
    );
    assert!(decision.next.common.terminal.is_none());
}

#[test]
fn mutation_send_receipt_from_verification_chain_is_rejected() {
    // ⚠️ Confirmation `SendVerify` You don't have to move the registration.
    let state = armed();
    // ⚠️ You don't have to guess. `SendVerify` In a machine, the change is not made by any
    // transition, and this is the first thing that can be said for sure about it.
    let err = decide(
        &state,
        DomainInput::Receipt {
            effect: PlannedEffect::at(EffectSpec::SendVerify, state.common.transition_index, 0),
            receipt: EffectReceipt::SendAccepted,
        },
    );
    // ⚠️ Failure to look: not `NotSpecified`Not "the wrong place" but "this entrance is not
    // from this car" (C-03).
    assert!(matches!(
        err,
        Err(TransitionError::InvalidInputForOperation { .. })
    ));
}

#[test]
fn metadata_failure_after_arm_goes_to_reconciliation_with_lock() {
    // B-02: Metadata recording failed - reconciliation, area blocked.
    let state = armed();
    let mut state = decide(
        &state,
        receipt_of(
            &state,
            EffectSpec::SendMutatingCommand,
            EffectReceipt::MutationOutcomeUnknown,
        ),
    )
    .expect("переход допустим")
    .next;
    assert_eq!(state.common.phase, OperationPhase::Reconciling);

    // The positive result here is artificial: we check only the transition of the machine and
    // the address of the really published recording effect.
    state = decide(
        &state,
        DomainInput::Reconciliation(ReconciliationVerdict::DesiredStateConfirmed(verified(2))),
    )
    .unwrap()
    .next;
    let decision = decide(
        &state,
        receipt_of(
            &state,
            EffectSpec::CommitDesiredRegistryAndJournal,
            EffectReceipt::MetadataFailed,
        ),
    )
    .expect("переход допустим");
    assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
    assert!(
        decision
            .effects
            .iter()
            .any(|e| e.spec == EffectSpec::QuarantineScope)
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RecoverMetadataCommit)
    );
    assert!(!decision.effects.iter().any(|effect| matches!(
        effect.spec,
        EffectSpec::PersistNeedsReconciliation
            | EffectSpec::CommitDesiredRegistryAndJournal
            | EffectSpec::CommitNoChangeAndJournal
            | EffectSpec::SendMutatingCommand
    )));
    for verdict in [
        ReconciliationVerdict::DesiredStateConfirmed(verified(2)),
        ReconciliationVerdict::PreStateConfirmed(verified(1)),
        ReconciliationVerdict::Indeterminate(ReconciliationFailure::Unsupported),
    ] {
        let before = decision.next.clone();
        assert!(matches!(
            decide(&decision.next, DomainInput::Reconciliation(verdict)),
            Err(TransitionError::NotSpecified { .. })
        ));
        assert_eq!(decision.next, before);
    }
    for input in [
        DomainInput::WorkerLost,
        DomainInput::OwnerRevoked,
        DomainInput::StorageUnavailable,
    ] {
        let interrupted = decide(&decision.next, input).unwrap();
        assert_eq!(interrupted.next.common.phase, OperationPhase::Reconciling);
        assert!(
            interrupted
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::RecoverMetadataCommit)
        );
        assert!(!interrupted.effects.iter().any(|effect| matches!(
            effect.spec,
            EffectSpec::PersistNeedsReconciliation
                | EffectSpec::CommitDesiredRegistryAndJournal
                | EffectSpec::CommitNoChangeAndJournal
                | EffectSpec::SendMutatingCommand
        )));
    }
}

#[test]
fn mutation_receipt_requires_an_issued_effect_and_matching_kind() {
    let state = decide(&mutation_state(), DomainInput::Authorized)
        .unwrap()
        .next;
    let issued = state.common.pending_effects[0];
    for effect in [
        PlannedEffect::at(
            EffectSpec::PersistIntent,
            issued.transition.next().unwrap(),
            0,
        ),
        PlannedEffect::at(
            EffectSpec::PersistDispatchArmed,
            issued.transition,
            issued.position,
        ),
    ] {
        assert!(matches!(
            decide(
                &state,
                DomainInput::Receipt {
                    effect,
                    receipt: EffectReceipt::IntentDurable
                }
            ),
            Err(TransitionError::ReceiptMismatch)
        ));
    }
    assert!(matches!(
        decide(
            &state,
            DomainInput::Receipt {
                effect: issued,
                receipt: EffectReceipt::MetadataDurable
            }
        ),
        Err(TransitionError::ReceiptMismatch)
    ));
}

#[test]
fn storage_unavailable_does_not_claim_durable_uncertainty() {
    // B-02: If you don't have a record, it's just a lockdown, no promises.
    let state = armed();
    let decision = decide(&state, DomainInput::StorageUnavailable).expect("переход допустим");
    assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
    assert_eq!(decision.effects.len(), 1);
    assert_eq!(decision.effects[0].spec, EffectSpec::QuarantineScope);
}

#[test]
fn interruption_of_metadata_commit_never_reissues_it() {
    for input in [
        DomainInput::WorkerLost,
        DomainInput::OwnerRevoked,
        DomainInput::StorageUnavailable,
    ] {
        let mut state = armed();
        state.common.phase = OperationPhase::Reconciling;
        // An artificial positive result only checks the effect address; a full-fledged
        // independent checker has not yet been implemented.
        state = decide(
            &state,
            DomainInput::Reconciliation(ReconciliationVerdict::DesiredStateConfirmed(verified(2))),
        )
        .unwrap()
        .next;
        let commit = *state
            .common
            .pending_effects
            .iter()
            .find(|effect| effect.spec == EffectSpec::CommitDesiredRegistryAndJournal)
            .unwrap();
        let decision = decide(&state, input).unwrap();
        assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
        assert!(!decision.next.common.pending_effects.contains(&commit));
        assert!(
            decision
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::RecoverMetadataCommit)
        );
        assert!(
            !decision
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::PersistNeedsReconciliation)
        );
        assert!(
            !decision
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::CommitDesiredRegistryAndJournal)
        );
        assert!(matches!(
            decide(
                &decision.next,
                DomainInput::Receipt {
                    effect: commit,
                    receipt: EffectReceipt::MetadataCommitted
                }
            ),
            Err(TransitionError::ReceiptMismatch)
        ));
    }
}

#[test]
fn post_arm_failure_or_loss_never_leaves_a_retryable_send() {
    for input in [
        DomainInput::ProviderFailed,
        DomainInput::WorkerLost,
        DomainInput::OwnerRevoked,
        DomainInput::StorageUnavailable,
    ] {
        let state = armed();
        assert!(
            state
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
        );
        let decision = decide(&state, input).unwrap();
        assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
        assert!(
            decision
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::QuarantineScope)
        );
        assert!(
            !decision
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
        );
        assert!(
            !decision
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::ReleaseResources)
        );
    }
}

#[test]
fn post_arm_cancellation_requests_stop_and_reconciliation() {
    for input in [
        DomainInput::OwnerRevoked,
        DomainInput::OwnerDetached,
        DomainInput::DeadlineElapsed,
        DomainInput::CancelRequested,
    ] {
        let state = armed();
        let decision = decide(&state, input).unwrap();
        assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
        assert!(decision.next.common.cancel_requested);
        for spec in [
            EffectSpec::RequestCancel,
            EffectSpec::PersistNeedsReconciliation,
            EffectSpec::QuarantineScope,
        ] {
            assert!(decision.effects.iter().any(|effect| effect.spec == spec));
        }
        assert!(
            !decision
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
        );
    }
}

#[test]
fn pre_arm_cancellation_does_not_leave_grant_revalidation_pending() {
    let state = advance(
        mutation_state(),
        &[
            (EffectSpec::PersistIntent, EffectReceipt::IntentDurable),
            (EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        ],
    );
    let cancelled = decide(&state, DomainInput::CancelRequested).unwrap().next;
    assert_eq!(cancelled.common.phase, OperationPhase::Finalizing);
    assert!(cancelled.common.cancel_requested);
    assert!(
        !cancelled
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RevalidateGrant)
    );
}

#[test]
fn pre_arm_cancellation_waits_for_pending_lease_result() {
    for receipt in [EffectReceipt::LeaseGranted, EffectReceipt::LeaseDenied] {
        let waiting = advance(
            mutation_state(),
            &[(EffectSpec::PersistIntent, EffectReceipt::IntentDurable)],
        );
        let acquire = *waiting
            .common
            .pending_effects
            .iter()
            .find(|effect| effect.spec == EffectSpec::AcquireResources)
            .unwrap();
        let cancelled = decide(&waiting, DomainInput::CancelRequested).unwrap();
        assert_eq!(cancelled.next.common.phase, OperationPhase::Cancelling);
        assert!(cancelled.effects.is_empty());
        assert!(cancelled.next.common.pending_effects.contains(&acquire));
        let repeated = decide(&cancelled.next, DomainInput::OwnerRevoked).unwrap();
        assert_eq!(repeated.next.common.phase, OperationPhase::Cancelling);
        assert!(repeated.effects.is_empty());
        let result = decide(
            &repeated.next,
            DomainInput::Receipt {
                effect: acquire,
                receipt,
            },
        )
        .unwrap();
        assert_eq!(result.next.common.phase, OperationPhase::Finalizing);
        assert_eq!(
            result.next.common.terminal,
            Some(TerminalOutcome::Cancelled)
        );
        let expected = if receipt == EffectReceipt::LeaseGranted {
            EffectSpec::ReleaseResources
        } else {
            EffectSpec::RecordTerminal
        };
        assert_eq!(result.effects.len(), 1);
        assert_eq!(result.effects[0].spec, expected);
        assert!(
            !result
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| matches!(
                    effect.spec,
                    EffectSpec::PersistDispatchArmed | EffectSpec::SendMutatingCommand
                ))
        );
    }
}

#[test]
fn successful_reconciliation_commits_via_registry_action() {
    // B-01: Nothing has changed, too. `Committing`.
    let mut state = armed();
    state.common.phase = OperationPhase::Reconciling;
    let decision = decide(
        &state,
        DomainInput::Reconciliation(ReconciliationVerdict::PreStateConfirmed(verified(1))),
    )
    .expect("переход допустим");
    assert_eq!(decision.next.common.phase, OperationPhase::Committing);
    assert_eq!(
        decision.effects[0].spec,
        EffectSpec::CommitNoChangeAndJournal
    );
}

#[test]
fn reconciliation_commit_requires_the_matching_typed_durable_receipt() {
    for (verdict, spec, receipt, foreign) in [
        (
            ReconciliationVerdict::DesiredStateConfirmed(verified(2)),
            EffectSpec::CommitDesiredRegistryAndJournal,
            EffectReceipt::MetadataCommitted,
            EffectReceipt::MetadataNoChange,
        ),
        (
            ReconciliationVerdict::PreStateConfirmed(verified(1)),
            EffectSpec::CommitNoChangeAndJournal,
            EffectReceipt::MetadataNoChange,
            EffectReceipt::MetadataCommitted,
        ),
    ] {
        let mut state = armed();
        state.common.phase = OperationPhase::Reconciling;
        let committing = decide(&state, DomainInput::Reconciliation(verdict))
            .unwrap()
            .next;
        assert_eq!(committing.mutation().unwrap().reconciliation, Some(verdict));
        let effect = *committing
            .common
            .pending_effects
            .iter()
            .find(|effect| effect.spec == spec)
            .unwrap();
        for invalid in [foreign, EffectReceipt::MetadataDurable] {
            assert!(matches!(
                decide(
                    &committing,
                    DomainInput::Receipt {
                        effect,
                        receipt: invalid
                    }
                ),
                Err(TransitionError::ReceiptMismatch)
            ));
        }
        let mut inconsistent = committing.clone();
        inconsistent.body.mutation_mut().unwrap().registry = Some(match spec {
            EffectSpec::CommitDesiredRegistryAndJournal => RegistryAction::RecordNoChange,
            EffectSpec::CommitNoChangeAndJournal => RegistryAction::ApplyDesiredState,
            _ => unreachable!(),
        });
        assert!(matches!(
            decide(&inconsistent, DomainInput::Receipt { effect, receipt }),
            Err(TransitionError::NotSpecified { .. })
        ));
        let mut missing_evidence = committing.clone();
        missing_evidence.body.mutation_mut().unwrap().reconciliation = None;
        assert!(matches!(
            decide(&missing_evidence, DomainInput::Receipt { effect, receipt }),
            Err(TransitionError::ReceiptMismatch)
        ));
        let failed = decide(
            &committing,
            DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::MetadataFailed,
            },
        )
        .unwrap();
        assert_eq!(failed.next.common.phase, OperationPhase::Reconciling);
        assert_eq!(failed.next.mutation().unwrap().reconciliation, None);
        let confirmed = decide(&committing, DomainInput::Receipt { effect, receipt }).unwrap();
        assert_eq!(confirmed.next.common.phase, OperationPhase::Finalizing);
        assert!(
            confirmed
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::ReleaseResources)
        );
    }
}

#[test]
fn direct_reconciling_to_terminal_is_impossible() {
    // B-01: no direct transition, only through `Committing`.
    let mut state = armed();
    state.common.phase = OperationPhase::Reconciling;
    let decision = decide(
        &state,
        DomainInput::Reconciliation(ReconciliationVerdict::DesiredStateConfirmed(verified(2))),
    )
    .expect("переход допустим");
    assert_ne!(decision.next.common.phase, OperationPhase::Terminal);
    assert!(decision.next.common.terminal.is_none());
}

#[test]
fn indeterminate_verdict_blocks_commit() {
    let mut state = armed();
    state.common.phase = OperationPhase::Reconciling;
    let decision = decide(
        &state,
        DomainInput::Reconciliation(ReconciliationVerdict::Indeterminate(
            ReconciliationFailure::Unsupported,
        )),
    )
    .expect("переход допустим");
    assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
    assert_eq!(
        decision.next.common.terminal,
        Some(TerminalOutcome::NeedsReconciliation)
    );
    assert!(!decision.effects.iter().any(|e| matches!(
        e.spec,
        EffectSpec::CommitDesiredRegistryAndJournal | EffectSpec::CommitNoChangeAndJournal
    )));
}

#[test]
fn provider_no_change_claim_cannot_commit_after_arm() {
    for phase in [OperationPhase::InFlight, OperationPhase::ValidatingChange] {
        let mut state = armed();
        state.common.phase = phase;
        let decision = decide(&state, DomainInput::ProviderNoChange).unwrap();
        assert_eq!(decision.next.common.phase, OperationPhase::Reconciling);
        assert_eq!(
            decision.next.common.terminal,
            Some(TerminalOutcome::NeedsReconciliation)
        );
        assert!(decision.next.mutation().unwrap().registry.is_none());
        assert!(
            decision
                .effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::QuarantineScope)
        );
        assert!(!decision.effects.iter().any(|effect| matches!(
            effect.spec,
            EffectSpec::CommitDesiredRegistryAndJournal | EffectSpec::CommitNoChangeAndJournal
        )));
        assert!(
            !decision
                .next
                .common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
        );
    }
}

#[test]
fn registry_action_is_recorded_for_both_successful_verdicts() {
    for (verdict, action) in [
        (
            ReconciliationVerdict::DesiredStateConfirmed(verified(2)),
            RegistryAction::ApplyDesiredState,
        ),
        (
            ReconciliationVerdict::PreStateConfirmed(verified(1)),
            RegistryAction::RecordNoChange,
        ),
    ] {
        let mut state = armed();
        state.common.phase = OperationPhase::Reconciling;
        let outcome = carosign_engine::machines::enrollment::step(
            &state,
            carosign_engine::machines::enrollment::MutationInput::Reconciled(verdict),
        )
        .expect("переход допустим");
        assert_eq!(outcome.registry, Some(action));
    }
}

#[test]
fn completed_enrollment_must_echo_the_planned_identity() {
    // ⚠️ Document 34: Completed registration must repeat the planned one exactly; discrepancy
    // or absence is the unknown, not “nothing.”
    use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentRef};
    use carosign_core::ids::EnrollmentId;

    let planned = EnrollmentRef {
        id: EnrollmentId::from_bytes([8u8; 16]).expect("валид"),
        authenticator: carosign_core::ids::AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
        generation: EnrollmentGeneration::FIRST,
    };
    let mut state = armed();
    if let Some(mutation) = state.body.mutation_mut() {
        mutation.planned_new = Some(planned);
    }

    let echo = decide(&state, DomainInput::ProviderCompleted { echoed: planned })
        .expect("переход допустим");
    assert_eq!(echo.next.common.phase, OperationPhase::ValidatingChange);
    assert!(
        !echo
            .next
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::SendMutatingCommand)
    );

    let wrong = decide(
        &state,
        DomainInput::ProviderCompleted {
            echoed: EnrollmentRef {
                id: EnrollmentId::from_bytes([7u8; 16]).expect("валид"),
                ..planned
            },
        },
    )
    .expect("переход допустим");
    assert_eq!(wrong.next.common.phase, OperationPhase::Reconciling);

    let missing =
        decide(&state, DomainInput::ProviderCompletedWithoutEcho).expect("переход допустим");
    assert_eq!(missing.next.common.phase, OperationPhase::Reconciling);
}
