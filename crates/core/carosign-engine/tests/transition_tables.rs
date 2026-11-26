//! Engine checks: tables of document 04, conformance E-01...E-07.
//!
//! ⚠️ No verification here works with hardware: the engine is a pure function, and this is
//! its main property, which must remain verifiable on the host.

use carosign_core::ids::EvidenceId;

use carosign_engine::tests_support::{context, initial};
use carosign_engine::{
    Decision, DomainInput, EffectReceipt, EffectSpec, OperationPhase, OperationState,
    PlannedEffect, TerminalOutcome, TransitionError, TransitionIndex, WorkerRef, decide,
};

/// Runs the sequence of inputs, falling at any error.
fn run(inputs: Vec<DomainInput>) -> OperationState {
    let mut state = initial();
    for input in inputs {
        state = decide(&state, input).expect("переход разрешён").next;
    }
    state
}

/// Prepares the effect to confirm the transition number and position.
fn effect(next_index: u64, spec: EffectSpec) -> PlannedEffect {
    PlannedEffect::at(spec, TransitionIndex::new(next_index), 0)
}

/// Confirmation addressed to the next transition effect.
fn receipt(next_index: u64, spec: EffectSpec, receipt: EffectReceipt) -> DomainInput {
    DomainInput::Receipt {
        effect: effect(next_index, spec),
        receipt,
    }
}

/// Sending a check: its own type of entry (C-03).
fn verification_sent(next_index: u64) -> DomainInput {
    let _ = next_index;
    DomainInput::VerificationSent
}

fn evidence() -> EvidenceId {
    EvidenceId::from_bytes([9u8; 16]).expect("валид")
}

#[test]
fn authorized_asks_for_resources_only() {
    let decision: Decision = decide(&initial(), DomainInput::Authorized).expect("допущено");
    assert_eq!(decision.next.common.phase, OperationPhase::WaitingLease);
    assert_eq!(decision.effects.len(), 1);
    assert_eq!(decision.effects[0].spec, EffectSpec::AcquireResources);
    assert_eq!(decision.next.common.transition_index.value(), 1);
}

#[test]
fn effect_keys_are_unique_per_transition() {
    let decision = decide(&initial(), DomainInput::Authorized).expect("допущено");
    let key = &decision.effects[0];
    assert_eq!(key.transition, decision.next.common.transition_index);
    assert_eq!(key.position, 0);
}

#[test]
fn progress_is_coalesced_until_publication_is_acknowledged() {
    let state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
    ]);
    let first = decide(&state, DomainInput::Progress).unwrap();
    assert_eq!(first.effects.len(), 1);
    let progress = first.effects[0];
    assert_eq!(progress.spec, EffectSpec::PublishProgress);
    let second = decide(&first.next, DomainInput::Progress).unwrap();
    assert!(second.effects.is_empty());
    assert!(
        second
            .next
            .common
            .pending_effects
            .iter()
            .any(|effect| effect.spec == EffectSpec::SendVerify)
    );
    let acknowledged = decide(
        &second.next,
        DomainInput::Receipt {
            effect: progress,
            receipt: EffectReceipt::ProgressPublished,
        },
    )
    .unwrap();
    assert!(!acknowledged.next.common.pending_effects.contains(&progress));
    assert_eq!(
        decide(&acknowledged.next, DomainInput::Progress)
            .unwrap()
            .effects[0]
            .spec,
        EffectSpec::PublishProgress
    );
}

#[test]
fn matched_goes_to_evidence_check_not_success() {
    let state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
        verification_sent(3),
        DomainInput::ProviderMatched(evidence()),
    ]);
    assert_eq!(state.common.phase, OperationPhase::EvaluatingEvidence);
    assert_eq!(state.common.terminal, None);
}

#[test]
fn accepted_evidence_with_valid_owner_is_accepted_factor() {
    let state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
        verification_sent(3),
        DomainInput::ProviderMatched(evidence()),
        // The 5th transition puts VerifyEvidence; we're sending him confirmation.
        receipt(
            5,
            EffectSpec::VerifyEvidence,
            EffectReceipt::EvidenceAccepted,
        ),
        // Crossing 6. RevalidateOwner.
        receipt(6, EffectSpec::RevalidateOwner, EffectReceipt::OwnerValid),
    ]);
    assert_eq!(state.common.terminal, Some(TerminalOutcome::FactorAccepted));
    assert_eq!(state.common.phase, OperationPhase::Finalizing);
}

#[test]
fn revoked_owner_suppresses_success() {
    // E-01: matched, proof accepted, but owner recalled - no success delivered.
    let mut state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
        verification_sent(3),
        DomainInput::ProviderMatched(evidence()),
        receipt(
            5,
            EffectSpec::VerifyEvidence,
            EffectReceipt::EvidenceAccepted,
        ),
    ]);
    assert_eq!(state.common.terminal, None);
    state = decide(&state, DomainInput::OwnerRevoked)
        .expect("отзыв допустим")
        .next;
    assert_eq!(state.common.terminal, Some(TerminalOutcome::Cancelled));
    assert!(state.common.cancel_requested);
    assert_eq!(state.common.phase, OperationPhase::Cancelling);
}

#[test]
fn owner_revoked_while_checking_owner_wins_over_acceptance() {
    let state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
        verification_sent(3),
        DomainInput::ProviderMatched(evidence()),
        receipt(
            5,
            EffectSpec::VerifyEvidence,
            EffectReceipt::EvidenceAccepted,
        ),
        receipt(6, EffectSpec::RevalidateOwner, EffectReceipt::OwnerRevoked),
    ]);
    assert_eq!(state.common.terminal, Some(TerminalOutcome::Cancelled));
}

#[test]
fn not_matched_is_a_normal_outcome() {
    let state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
        verification_sent(3),
        DomainInput::ProviderNotMatched,
    ]);
    assert_eq!(state.common.terminal, Some(TerminalOutcome::NotMatched));
}

#[test]
fn stale_epoch_is_rejected_without_change() {
    // E-02: Entry of an outdated era is rejected, status unchanged.
    let mut state = initial();
    state = decide(&state, DomainInput::Authorized)
        .expect("допущено")
        .next;
    let epoch = state.common.context.authenticator.device_epoch;
    state.common.worker = Some(WorkerRef {
        instance: carosign_core::ids::WorkerInstanceId::from_bytes([1u8; 16]).expect("валид"),
        device_epoch: epoch,
    });
    let stranger = epoch.next().expect("не переполнено");
    let before = state.clone();
    let result = decide(&state, DomainInput::DeviceEpochChanged(stranger));
    assert_eq!(result, Err(TransitionError::StaleFencing));
    assert_eq!(state, before);
}

#[test]
fn terminal_state_does_not_accept_more_input() {
    let mut state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
        receipt(2, EffectSpec::RevalidateGrant, EffectReceipt::GrantValid),
        verification_sent(3),
        DomainInput::ProviderNotMatched,
        receipt(
            5,
            EffectSpec::ReleaseResources,
            EffectReceipt::CleanupConfirmed,
        ),
        receipt(
            6,
            EffectSpec::RecordTerminal,
            EffectReceipt::VerificationTerminalRecorded,
        ),
    ]);
    assert_eq!(state.common.phase, OperationPhase::Terminal);
    assert_eq!(
        decide(&state, DomainInput::Progress),
        Err(TransitionError::AlreadyTerminal)
    );
    state.common.transition_index = TransitionIndex::INITIAL;
}

#[test]
fn replay_is_deterministic() {
    // E-07: The same inputs give the same solution.
    let inputs = vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
    ];
    let first = run(inputs.clone());
    let second = run(inputs);
    assert_eq!(first, second);
}

#[test]
fn cancel_before_arming_is_safe_no_change() {
    // E-04: Cancellation prior to clearance - confirmed nothing sent.
    let state = run(vec![
        DomainInput::Authorized,
        receipt(1, EffectSpec::AcquireResources, EffectReceipt::LeaseGranted),
    ]);
    let decision = decide(&state, DomainInput::CancelRequested).expect("отмена допустима");
    assert_eq!(decision.next.common.phase, OperationPhase::Cancelling);
    assert_eq!(
        carosign_engine::recovery::action_for(state.common.phase),
        carosign_engine::recovery::RecoveryAction::SafeNoChange
    );
    let _ = context();
}
