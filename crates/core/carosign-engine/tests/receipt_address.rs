//! Engine/executor joint: Confirmation is addressed to the **published* effect.
//!
//! This test exercises the full receipt-address path. An earlier off-by-one accepted only
//! address N-1 for an effect at N, leaving the operation in `WaitingLease`.
//!
//! ⚠️ The test is done the way the performer does it: it takes a place from the published
//! effect, rather than deduces it itself, otherwise the test would confirm its own fiction.

use carosign_engine::tests_support::initial;
use carosign_engine::{
    DomainInput, EffectReceipt, EffectSpec, OperationPhase, PlannedEffect, TransitionError,
    TransitionIndex, decide,
};

/// Confirms the effect at the place in which it was published (as a performer).
fn confirm(
    state: &carosign_engine::OperationState,
    position: usize,
    receipt: EffectReceipt,
) -> carosign_engine::OperationState {
    let effect = state.common.pending_effects[position];
    decide(state, DomainInput::Receipt { effect, receipt })
        .expect("подтверждение изданного эффекта обязано приниматься")
        .next
}

#[test]
fn receipts_of_issued_effects_are_accepted() {
    let mut state = initial();

    // `Authorized` publisher `AcquireResources` Place 1.
    state = decide(&state, DomainInput::Authorized)
        .expect("допуск")
        .next;
    assert_eq!(state.common.phase, OperationPhase::WaitingLease);
    assert_eq!(state.common.transition_index, TransitionIndex::new(1));
    assert_eq!(
        state.common.pending_effects[0].transition,
        TransitionIndex::new(1)
    );

    // ⚠️ Confirmation carries place 1 - that which issued the transition, not 0.
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    assert_eq!(state.common.phase, OperationPhase::ReadyToDispatch);
    assert_eq!(
        state.common.pending_effects[0].spec,
        EffectSpec::RevalidateGrant
    );

    state = confirm(&state, 0, EffectReceipt::GrantValid);
    assert_eq!(state.common.phase, OperationPhase::Running);
    assert_eq!(state.common.pending_effects[0].spec, EffectSpec::SendVerify);
}

#[test]
fn verification_dispatch_binding_is_scoped_to_pending_effect_worker_and_epoch() {
    use carosign_core::device::DeviceEpoch;
    use carosign_core::ids::{CommandId, LeaseId, WorkerInstanceId};
    use carosign_engine::WorkerRef;

    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    let effect = state.common.pending_effects[0];
    let worker = WorkerRef {
        instance: WorkerInstanceId::from_bytes([7; 16]).unwrap(),
        device_epoch: state.common.context.authenticator.device_epoch,
    };
    let command = CommandId::from_bytes([8; 16]).unwrap();
    let bind = |worker, effect| DomainInput::BindVerificationDispatch {
        effect,
        lease: LeaseId::from_bytes([9; 16]).unwrap(),
        worker,
        command,
    };
    let mut stale = worker;
    stale.device_epoch = DeviceEpoch::new(worker.device_epoch.value() + 1).unwrap();
    assert!(matches!(
        decide(&state, bind(stale, effect)),
        Err(TransitionError::StaleFencing)
    ));
    let wrong_effect = PlannedEffect::at(
        EffectSpec::SendVerify,
        TransitionIndex::new(effect.transition.value() + 1),
        effect.position,
    );
    assert!(decide(&state, bind(worker, wrong_effect)).is_err());
    let decision = decide(&state, bind(worker, effect)).unwrap();
    assert!(decision.effects.is_empty());
    assert_eq!(decision.next.common.worker, Some(worker));
    assert_eq!(decision.next.common.command.unwrap().id, command);
    assert_eq!(
        decision.next.common.command.unwrap().transition_index,
        effect.transition
    );
    assert!(decide(&decision.next, bind(worker, effect)).is_err());
    let after_ack = decide(
        &decision.next,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::SendAccepted,
        },
    )
    .unwrap()
    .next;
    assert!(!after_ack.common.pending_effects.contains(&effect));
    assert!(decide(&after_ack, bind(worker, effect)).is_err());
}

#[test]
fn provider_match_without_send_ack_does_not_leave_send_pending() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    let send = state.common.pending_effects[0];
    let decision = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap();
    assert_eq!(
        decision.next.common.phase,
        OperationPhase::EvaluatingEvidence
    );
    assert!(!decision.next.common.pending_effects.contains(&send));
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::VerifyEvidence)
    );
}

#[test]
fn receipt_with_the_old_address_is_refused() {
    // ⚠️ Place N-1 is the old, erroneous rule, and it has to be a failure, not a quiet
    // reception, or it will come back unnoticed.
    // The previous error is visible only from the second confirmation: the first N-1 has a 0
    // - a random coincidence on which it was held.
    let state = decide(&initial(), DomainInput::Authorized)
        .expect("допуск")
        .next;
    let state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    let stale = carosign_engine::PlannedEffect::at(
        EffectSpec::RevalidateGrant,
        TransitionIndex::new(1),
        0,
    );
    let result = decide(
        &state,
        DomainInput::Receipt {
            effect: stale,
            receipt: EffectReceipt::GrantValid,
        },
    );
    assert!(
        result.is_err(),
        "подтверждение по прежнему месту обязано быть отказом"
    );
}

#[test]
fn dispatch_receipts_keep_not_sent_and_unknown_distinct() {
    use carosign_core::operation::verification::VerificationFailure;

    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    let effect = state.common.pending_effects[0];
    let not_sent = decide(
        &state,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::DispatchNotSent,
        },
    )
    .unwrap();
    assert_eq!(
        not_sent.next.common.failure,
        Some(VerificationFailure::DispatchNotSent)
    );
    assert_eq!(
        not_sent.next.common.terminal,
        Some(carosign_engine::TerminalOutcome::Failed)
    );
    assert!(
        not_sent
            .effects
            .iter()
            .any(|e| e.spec == EffectSpec::ReleaseResources)
    );
    assert!(
        !not_sent
            .effects
            .iter()
            .any(|e| e.spec == EffectSpec::QuarantineScope)
    );

    let unknown = decide(
        &state,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::DispatchOutcomeUnknown,
        },
    )
    .unwrap();
    assert_eq!(unknown.next.common.phase, OperationPhase::Cancelling);
    assert_eq!(
        unknown.next.common.failure,
        Some(VerificationFailure::DispatchOutcomeUnknown)
    );
    for spec in [
        EffectSpec::RequestCancel,
        EffectSpec::SuppressResult,
        EffectSpec::QuarantineScope,
    ] {
        assert!(unknown.effects.iter().any(|e| e.spec == spec), "{spec:?}");
    }
    assert!(
        !unknown
            .effects
            .iter()
            .any(|e| e.spec == EffectSpec::ReleaseResources)
    );
    assert!(decide(&unknown.next, DomainInput::ProviderNotMatched).is_err());

    let quarantine = *unknown
        .effects
        .iter()
        .find(|e| e.spec == EffectSpec::QuarantineScope)
        .unwrap();
    let quarantined = decide(
        &unknown.next,
        DomainInput::Receipt {
            effect: quarantine,
            receipt: EffectReceipt::ScopeQuarantined,
        },
    )
    .unwrap();
    assert_eq!(quarantined.next.common.phase, OperationPhase::Cancelling);
    assert!(
        !quarantined
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::ReleaseResources
                || effect.spec == EffectSpec::RecordTerminal)
    );

    let cancel = unknown
        .effects
        .iter()
        .find(|e| e.spec == EffectSpec::RequestCancel)
        .unwrap();
    let cleanup = decide(
        &unknown.next,
        DomainInput::Receipt {
            effect: *cancel,
            receipt: EffectReceipt::CancelDispatched,
        },
    )
    .unwrap();
    assert!(
        cleanup
            .effects
            .iter()
            .any(|e| e.spec == EffectSpec::ReleaseResources)
    );
}

#[test]
fn suppression_invalidates_pending_verification_and_late_receipts() {
    for input in [
        DomainInput::OwnerRevoked,
        DomainInput::OwnerDetached,
        DomainInput::DeadlineElapsed,
        DomainInput::CancelRequested,
    ] {
        let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
        state = confirm(&state, 0, EffectReceipt::LeaseGranted);
        state = confirm(&state, 0, EffectReceipt::GrantValid);
        let send = state.common.pending_effects[0];
        let cancelled = decide(&state, input).unwrap().next;
        assert!(!cancelled.common.pending_effects.contains(&send));
        assert!(
            decide(
                &cancelled,
                DomainInput::Receipt {
                    effect: send,
                    receipt: EffectReceipt::SendAccepted,
                },
            )
            .is_err()
        );
        assert!(decide(&cancelled, DomainInput::ProviderNotMatched).is_err());
    }
}

#[test]
fn terminal_record_requires_verification_confirmation() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseDenied);
    let effect = state.common.pending_effects[0];
    assert_eq!(effect.spec, EffectSpec::RecordTerminal);
    for receipt in [EffectReceipt::MetadataFailed, EffectReceipt::LeaseGranted] {
        assert!(decide(&state, DomainInput::Receipt { effect, receipt }).is_err());
    }
    let completed = decide(
        &state,
        DomainInput::Receipt {
            effect,
            receipt: EffectReceipt::VerificationTerminalRecorded,
        },
    )
    .unwrap();
    assert_eq!(completed.next.common.phase, OperationPhase::Terminal);
}

#[test]
fn cancellation_receipt_does_not_accept_unrelated_observations() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state.common.lease = Some(carosign_core::ids::LeaseId::from_bytes([7; 16]).unwrap());
    let cancelled = decide(&state, DomainInput::CancelRequested).unwrap().next;
    let effect = *cancelled
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RequestCancel)
        .unwrap();
    assert!(
        decide(
            &cancelled,
            DomainInput::Receipt {
                effect,
                receipt: EffectReceipt::CleanupConfirmed,
            },
        )
        .is_err()
    );
}

#[test]
fn failure_or_worker_loss_invalidates_pending_send() {
    for input in [
        DomainInput::WorkerLost,
        DomainInput::VerificationNotSent,
        DomainInput::VerificationOutcomeUnknown,
    ] {
        let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
        state = confirm(&state, 0, EffectReceipt::LeaseGranted);
        state = confirm(&state, 0, EffectReceipt::GrantValid);
        let send = state.common.pending_effects[0];
        let failed = decide(&state, input).unwrap().next;
        assert!(!failed.common.pending_effects.contains(&send));
        assert!(
            decide(
                &failed,
                DomainInput::Receipt {
                    effect: send,
                    receipt: EffectReceipt::SendAccepted,
                },
            )
            .is_err()
        );
    }
}

#[test]
fn worker_loss_invalidates_undelivered_accepted_result() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    state = confirm(&state, 0, EffectReceipt::EvidenceAccepted);
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    state = confirm(&state, 0, EffectReceipt::CleanupConfirmed);
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    let delivery = *state
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::DeliverOutcome)
        .unwrap();
    let failed = decide(&state, DomainInput::WorkerLost).unwrap().next;
    assert!(!failed.common.pending_effects.contains(&delivery));
    let destroy = *failed
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::DestroyEvidence)
        .unwrap();
    let cleared = decide(
        &failed,
        DomainInput::Receipt {
            effect: destroy,
            receipt: EffectReceipt::EvidenceDestroyed,
        },
    )
    .unwrap()
    .next;
    assert!(cleared.body.verification().unwrap().evidence.is_none());
    assert!(
        cleared
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
    assert!(
        !cleared
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RecordTerminal)
    );
    assert!(
        decide(
            &failed,
            DomainInput::Receipt {
                effect: delivery,
                receipt: EffectReceipt::OwnerDeliveryRecorded,
            },
        )
        .is_err()
    );
}

#[test]
fn cancellation_invalidates_held_evidence_only_after_destroy_receipt() {
    for input in [
        DomainInput::OwnerRevoked,
        DomainInput::OwnerDetached,
        DomainInput::CancelRequested,
        DomainInput::DeadlineElapsed,
    ] {
        let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
        state = confirm(&state, 0, EffectReceipt::LeaseGranted);
        state = confirm(&state, 0, EffectReceipt::GrantValid);
        state = confirm(&state, 0, EffectReceipt::SendAccepted);
        state = decide(
            &state,
            DomainInput::ProviderMatched(
                carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap(),
            ),
        )
        .unwrap()
        .next;
        let verification = state
            .common
            .pending_effects
            .iter()
            .copied()
            .find(|effect| effect.spec == EffectSpec::VerifyEvidence)
            .unwrap();
        let cancelled = decide(&state, input).unwrap().next;
        assert_eq!(cancelled.common.phase, OperationPhase::Cancelling);
        assert!(
            cancelled
                .common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::ReleaseResources)
        );
        assert!(cancelled.body.verification().unwrap().evidence.is_some());
        let destroy = *cancelled
            .common
            .pending_effects
            .iter()
            .find(|effect| effect.spec == EffectSpec::DestroyEvidence)
            .unwrap();
        assert!(!cancelled.common.pending_effects.contains(&verification));
        assert!(
            decide(
                &cancelled,
                DomainInput::Receipt {
                    effect: verification,
                    receipt: EffectReceipt::EvidenceAccepted
                }
            )
            .is_err()
        );
        let cleared = decide(
            &cancelled,
            DomainInput::Receipt {
                effect: destroy,
                receipt: EffectReceipt::EvidenceDestroyed,
            },
        )
        .unwrap()
        .next;
        assert_eq!(cleared.common.phase, OperationPhase::Cancelling);
        assert!(cleared.body.verification().unwrap().evidence.is_none());
        assert!(!cleared.common.pending_effects.contains(&destroy));
    }
}

#[test]
fn revoked_owner_receipt_destroys_evidence_before_terminal_record() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    state = confirm(&state, 0, EffectReceipt::EvidenceAccepted);
    let before_cleanup = state.clone();
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    let release = *state
        .common
        .pending_effects
        .iter()
        .find(|e| e.spec == EffectSpec::ReleaseResources)
        .unwrap();
    state = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::CleanupConfirmed,
        },
    )
    .unwrap()
    .next;
    for original in [before_cleanup, state] {
        let owner = *original
            .common
            .pending_effects
            .iter()
            .find(|e| e.spec == EffectSpec::RevalidateOwner)
            .unwrap();
        let revoked = decide(
            &original,
            DomainInput::Receipt {
                effect: owner,
                receipt: EffectReceipt::OwnerRevoked,
            },
        )
        .unwrap()
        .next;
        assert!(revoked.body.verification().unwrap().evidence.is_some());
        assert!(
            !revoked
                .common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::RecordTerminal)
        );
        assert!(
            revoked
                .common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::DestroyEvidence)
        );
    }
}

#[test]
fn owner_detach_in_finalizing_waits_for_evidence_and_outstanding_cleanup() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    state = confirm(&state, 0, EffectReceipt::EvidenceAccepted);
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    let release = *state
        .common
        .pending_effects
        .iter()
        .find(|e| e.spec == EffectSpec::ReleaseResources)
        .unwrap();
    let already_clean = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::CleanupConfirmed,
        },
    )
    .unwrap()
    .next;
    for (original, cleanup_pending) in [(state, true), (already_clean, false)] {
        let cancelled = decide(&original, DomainInput::OwnerDetached).unwrap().next;
        assert_eq!(cancelled.common.phase, OperationPhase::Finalizing);
        let destroy = *cancelled
            .common
            .pending_effects
            .iter()
            .find(|e| e.spec == EffectSpec::DestroyEvidence)
            .unwrap();
        assert!(
            !cancelled
                .common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::RecordTerminal)
        );
        let after_destroy = decide(
            &cancelled,
            DomainInput::Receipt {
                effect: destroy,
                receipt: EffectReceipt::EvidenceDestroyed,
            },
        )
        .unwrap()
        .next;
        assert!(
            after_destroy
                .body
                .verification()
                .unwrap()
                .evidence
                .is_none()
        );
        let done = if cleanup_pending {
            assert!(
                !after_destroy
                    .common
                    .pending_effects
                    .iter()
                    .any(|e| e.spec == EffectSpec::RecordTerminal)
            );
            decide(
                &after_destroy,
                DomainInput::Receipt {
                    effect: release,
                    receipt: EffectReceipt::CleanupConfirmed,
                },
            )
            .unwrap()
            .next
        } else {
            after_destroy
        };
        assert!(
            done.common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::RecordTerminal)
        );
        assert_eq!(
            done.common.terminal,
            Some(carosign_engine::TerminalOutcome::Cancelled)
        );
    }
}

#[test]
fn receipt_is_matched_by_full_address_when_effect_kinds_repeat() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(&state, DomainInput::ProviderNotMatched)
        .unwrap()
        .next;
    let first = state.common.pending_effects[0];
    assert_eq!(first.spec, EffectSpec::ReleaseResources);

    // A condition with two waiting effects of the same kind checks the addressing regardless
    // of how the performer recovered the image.
    let second = carosign_engine::PlannedEffect::at(
        EffectSpec::ReleaseResources,
        first.transition,
        first.position + 1,
    );
    state.common.pending_effects.push(second);
    assert_ne!(first.position, second.position);
    let next = decide(
        &state,
        DomainInput::Receipt {
            effect: second,
            receipt: EffectReceipt::CleanupConfirmed,
        },
    )
    .unwrap()
    .next;
    assert!(next.common.pending_effects.contains(&first));
    assert!(!next.common.pending_effects.contains(&second));
}

#[test]
fn uncertain_cleanup_suppresses_pending_accepted_delivery() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    state = confirm(&state, 0, EffectReceipt::EvidenceAccepted);
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    assert!(
        !state
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
    );
    let release = *state
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::ReleaseResources)
        .unwrap();
    let decision = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::ResourceUncertain,
        },
    )
    .unwrap();
    assert_eq!(
        decision.next.common.terminal,
        Some(carosign_engine::TerminalOutcome::Unavailable)
    );
    assert!(
        !decision
            .next
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
    );
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
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DestroyEvidence)
    );
}

#[test]
fn uncertain_cleanup_invalidates_pending_terminal_receipt() {
    let mut state = initial();
    state.common.phase = OperationPhase::Finalizing;
    state.common.terminal = Some(carosign_engine::TerminalOutcome::Failed);
    state.common.transition_index = TransitionIndex::new(1);
    let release = PlannedEffect::at(
        EffectSpec::ReleaseResources,
        state.common.transition_index,
        0,
    );
    let terminal = PlannedEffect::at(EffectSpec::RecordTerminal, state.common.transition_index, 1);
    state.common.pending_effects.extend([release, terminal]);
    let decision = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::ResourceUncertain,
        },
    )
    .unwrap();
    assert!(!decision.next.common.pending_effects.contains(&terminal));
    assert!(matches!(
        decide(
            &decision.next,
            DomainInput::Receipt {
                effect: terminal,
                receipt: EffectReceipt::MetadataDurable
            }
        ),
        Err(TransitionError::ReceiptMismatch)
    ));
    assert!(
        decision
            .next
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
}

#[test]
fn cancellation_invalidates_prior_terminal_effect_and_does_not_duplicate_destroy() {
    let mut state = initial();
    state.common.phase = OperationPhase::Finalizing;
    state.common.transition_index = TransitionIndex::new(1);
    state.common.terminal = Some(carosign_engine::TerminalOutcome::FactorAccepted);
    let operation = state.common.context.operation_id;
    let carosign_engine::OperationBody::Verification(verification) = &mut state.body else {
        unreachable!()
    };
    verification.evidence = Some(carosign_core::evidence::EvidenceRef::new(
        carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap(),
        operation,
    ));
    let terminal = PlannedEffect::at(EffectSpec::RecordTerminal, state.common.transition_index, 0);
    state.common.pending_effects.push(terminal);
    let cancelled = decide(&state, DomainInput::OwnerDetached).unwrap().next;
    assert!(!cancelled.common.pending_effects.contains(&terminal));
    assert!(matches!(
        decide(
            &cancelled,
            DomainInput::Receipt {
                effect: terminal,
                receipt: EffectReceipt::MetadataDurable
            }
        ),
        Err(TransitionError::ReceiptMismatch)
    ));
    let repeated = decide(&cancelled, DomainInput::OwnerRevoked).unwrap().next;
    assert_eq!(
        repeated
            .common
            .pending_effects
            .iter()
            .filter(|effect| effect.spec == EffectSpec::DestroyEvidence)
            .count(),
        1
    );
}

#[test]
fn successful_delivery_requires_cleanup_and_current_owner() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    state = confirm(&state, 0, EffectReceipt::EvidenceAccepted);
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    assert_eq!(
        state.common.pending_effects[0].spec,
        EffectSpec::ReleaseResources
    );
    state = confirm(&state, 0, EffectReceipt::CleanupConfirmed);
    assert_eq!(
        state.common.pending_effects[0].spec,
        EffectSpec::RevalidateOwner
    );
    let revoked = confirm(&state, 0, EffectReceipt::OwnerRevoked);
    assert!(
        !revoked
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
    );
    state = confirm(&state, 0, EffectReceipt::OwnerValid);
    assert_eq!(
        state.common.pending_effects[0].spec,
        EffectSpec::DeliverOutcome
    );
    let mut disconnected = confirm(&state, 0, EffectReceipt::OwnerDisconnected);
    assert_eq!(
        disconnected.common.terminal,
        Some(carosign_engine::TerminalOutcome::Cancelled)
    );
    assert_eq!(
        disconnected.common.pending_effects[0].spec,
        EffectSpec::DestroyEvidence
    );
    disconnected = confirm(&disconnected, 0, EffectReceipt::EvidenceDestroyed);
    assert_eq!(
        disconnected.common.pending_effects[0].spec,
        EffectSpec::RecordTerminal
    );
    state = confirm(&state, 0, EffectReceipt::OwnerDeliveryRecorded);
    for late in [
        DomainInput::OwnerRevoked,
        DomainInput::OwnerDetached,
        DomainInput::DeadlineElapsed,
        DomainInput::WorkerLost,
    ] {
        let next = decide(&state, late).unwrap().next;
        assert_eq!(
            next.common.terminal,
            Some(carosign_engine::TerminalOutcome::FactorAccepted)
        );
        assert!(
            next.common
                .pending_effects
                .iter()
                .any(|effect| effect.spec == EffectSpec::DestroyEvidence)
        );
    }
    assert_eq!(
        state.common.pending_effects[0].spec,
        EffectSpec::DestroyEvidence
    );
    assert!(state.body.verification().unwrap().evidence.is_some());
    let held_evidence = state.body.verification().unwrap().evidence;
    state = confirm(&state, 0, EffectReceipt::EvidenceDestroyed);
    assert!(state.body.verification().unwrap().evidence.is_none());
    assert_eq!(
        state.common.pending_effects[0].spec,
        EffectSpec::RecordTerminal
    );
    let record = state.common.pending_effects[0];
    let mut held = state.clone();
    let carosign_engine::OperationBody::Verification(verification) = &mut held.body else {
        unreachable!()
    };
    verification.evidence = held_evidence;
    assert!(
        decide(
            &held,
            DomainInput::Receipt {
                effect: record,
                receipt: EffectReceipt::VerificationTerminalRecorded,
            }
        )
        .is_err()
    );
    let mut leased = state.clone();
    leased.common.lease = Some(carosign_core::ids::LeaseId::from_bytes([8; 16]).unwrap());
    assert!(
        decide(
            &leased,
            DomainInput::Receipt {
                effect: record,
                receipt: EffectReceipt::VerificationTerminalRecorded,
            }
        )
        .is_err()
    );
    state = confirm(&state, 0, EffectReceipt::VerificationTerminalRecorded);
    assert_eq!(state.common.phase, OperationPhase::Terminal);
    assert_eq!(
        state.common.terminal,
        Some(carosign_engine::TerminalOutcome::FactorAccepted)
    );
}

#[test]
fn cancel_running_verification_requests_worker_cancellation() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    let decision = decide(&state, DomainInput::CancelRequested).unwrap();
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RequestCancel)
    );
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::SuppressResult)
    );
}

#[test]
fn cancellation_during_cleanup_accepts_the_existing_cleanup_receipt() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(&state, DomainInput::ProviderNotMatched)
        .unwrap()
        .next;
    let release = state.common.pending_effects[0];
    state = decide(&state, DomainInput::OwnerRevoked).unwrap().next;
    let decision = decide(
        &state,
        DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::CleanupConfirmed,
        },
    )
    .unwrap();
    assert_eq!(decision.next.common.phase, OperationPhase::Finalizing);
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::RecordTerminal)
    );
    assert!(
        !decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
    );
}

#[test]
fn unrelated_receipt_cannot_confirm_result_suppression() {
    let state = decide(&initial(), DomainInput::CancelRequested)
        .unwrap()
        .next;
    let suppression = *state
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::SuppressResult)
        .unwrap();
    for receipt in [
        EffectReceipt::LeaseGranted,
        EffectReceipt::CleanupConfirmed,
        EffectReceipt::OwnerDeliveryRecorded,
        EffectReceipt::ScopeQuarantined,
    ] {
        let before = state.clone();
        assert_eq!(
            decide(
                &state,
                DomainInput::Receipt {
                    effect: suppression,
                    receipt
                }
            ),
            Err(TransitionError::ReceiptMismatch)
        );
        assert_eq!(state, before);
    }
}

#[test]
fn late_lease_after_cancellation_is_cleaned_up_before_terminal() {
    for (receipt, required) in [
        (EffectReceipt::LeaseGranted, EffectSpec::ReleaseResources),
        (EffectReceipt::LeaseDenied, EffectSpec::RecordTerminal),
    ] {
        let waiting = decide(&initial(), DomainInput::Authorized).unwrap().next;
        let acquire = *waiting
            .common
            .pending_effects
            .iter()
            .find(|effect| effect.spec == EffectSpec::AcquireResources)
            .unwrap();
        let cancelled = decide(&waiting, DomainInput::OwnerRevoked).unwrap().next;
        assert_eq!(cancelled.common.phase, OperationPhase::Cancelling);
        assert!(cancelled.common.pending_effects.contains(&acquire));
        let observed = decide(
            &cancelled,
            DomainInput::Receipt {
                effect: acquire,
                receipt,
            },
        )
        .unwrap();
        assert_eq!(observed.next.common.phase, OperationPhase::Finalizing);
        assert_eq!(
            observed.next.common.terminal,
            Some(carosign_engine::TerminalOutcome::Cancelled)
        );
        assert!(
            observed
                .effects
                .iter()
                .any(|effect| effect.spec == required)
        );
        if receipt == EffectReceipt::LeaseGranted {
            assert!(
                !observed
                    .effects
                    .iter()
                    .any(|effect| effect.spec == EffectSpec::RecordTerminal)
            );
            let release = *observed
                .effects
                .iter()
                .find(|effect| effect.spec == EffectSpec::ReleaseResources)
                .unwrap();
            let cleaned = decide(
                &observed.next,
                DomainInput::Receipt {
                    effect: release,
                    receipt: EffectReceipt::CleanupConfirmed,
                },
            )
            .unwrap();
            assert!(
                cleaned
                    .effects
                    .iter()
                    .any(|effect| effect.spec == EffectSpec::RecordTerminal)
            );
        }
    }
}

#[test]
fn rejected_evidence_is_failure_not_carosign_mismatch() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    let result = confirm(&state, 0, EffectReceipt::EvidenceRejected);
    assert_eq!(
        result.common.terminal,
        Some(carosign_engine::TerminalOutcome::Failed)
    );
    assert!(
        result
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DestroyEvidence)
    );
    assert!(
        result
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::ReleaseResources)
    );
    assert!(
        !result
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::DeliverOutcome)
    );
}

#[test]
fn rejected_evidence_and_cleanup_both_finish_before_terminal_record() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(
        &state,
        DomainInput::ProviderMatched(carosign_core::ids::EvidenceId::from_bytes([9; 16]).unwrap()),
    )
    .unwrap()
    .next;
    let state = confirm(&state, 0, EffectReceipt::EvidenceRejected);
    let release = *state
        .common
        .pending_effects
        .iter()
        .find(|e| e.spec == EffectSpec::ReleaseResources)
        .unwrap();
    let destroy = *state
        .common
        .pending_effects
        .iter()
        .find(|e| e.spec == EffectSpec::DestroyEvidence)
        .unwrap();
    for (first, second) in [
        (
            (release, EffectReceipt::CleanupConfirmed),
            (destroy, EffectReceipt::EvidenceDestroyed),
        ),
        (
            (destroy, EffectReceipt::EvidenceDestroyed),
            (release, EffectReceipt::CleanupConfirmed),
        ),
    ] {
        let middle = decide(
            &state,
            DomainInput::Receipt {
                effect: first.0,
                receipt: first.1,
            },
        )
        .unwrap();
        assert!(
            !middle
                .next
                .common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::RecordTerminal)
        );
        let done = decide(
            &middle.next,
            DomainInput::Receipt {
                effect: second.0,
                receipt: second.1,
            },
        )
        .unwrap();
        assert!(done.next.body.verification().unwrap().evidence.is_none());
        assert!(
            done.next
                .common
                .pending_effects
                .iter()
                .any(|e| e.spec == EffectSpec::RecordTerminal)
        );
    }
}

#[test]
fn worker_loss_quarantines_and_invalidates_pending_release() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    state = confirm(&state, 0, EffectReceipt::SendAccepted);
    state = decide(&state, DomainInput::ProviderNotMatched)
        .unwrap()
        .next;
    let release = state.common.pending_effects[0];
    let decision = decide(&state, DomainInput::WorkerLost).unwrap();
    assert!(
        decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
    assert!(!decision.next.common.pending_effects.contains(&release));
    assert!(
        !decision
            .effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::ReleaseResources)
    );
    assert!(
        decide(
            &decision.next,
            DomainInput::Receipt {
                effect: release,
                receipt: EffectReceipt::CleanupConfirmed,
            }
        )
        .is_err()
    );
}

#[test]
fn late_cancel_ack_after_worker_loss_cannot_restart_cleanup() {
    let mut state = decide(&initial(), DomainInput::Authorized).unwrap().next;
    state = confirm(&state, 0, EffectReceipt::LeaseGranted);
    state = confirm(&state, 0, EffectReceipt::GrantValid);
    let send = *state
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::SendVerify)
        .unwrap();
    state = decide(
        &state,
        DomainInput::Receipt {
            effect: send,
            receipt: EffectReceipt::DispatchOutcomeUnknown,
        },
    )
    .unwrap()
    .next;
    let cancel = *state
        .common
        .pending_effects
        .iter()
        .find(|effect| effect.spec == EffectSpec::RequestCancel)
        .unwrap();
    let lost = decide(&state, DomainInput::WorkerLost).unwrap().next;
    assert!(!lost.common.pending_effects.contains(&cancel));
    assert!(
        lost.common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::QuarantineScope)
    );
    assert!(matches!(
        decide(
            &lost,
            DomainInput::Receipt {
                effect: cancel,
                receipt: EffectReceipt::CancelDispatched
            }
        ),
        Err(TransitionError::ReceiptMismatch)
    ));
}
