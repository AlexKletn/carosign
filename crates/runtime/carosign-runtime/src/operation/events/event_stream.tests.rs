use super::super::*;
use super::EventStream;
use carosign_core::ids::OperationId;
use carosign_engine::{EffectSpec, OperationPhase, OperationState, TerminalOutcome};

fn state() -> OperationState {
    carosign_engine::tests_support::initial()
}
fn stream() -> EventStream {
    EventStream::new(state().common.context.operation_id)
}

#[test]
fn sequences_start_at_one_and_increase() {
    let mut stream = stream();
    assert_eq!(
        stream
            .publish(OperationPhase::New, EventKind::StateChanged)
            .unwrap(),
        1
    );
    assert_eq!(
        stream
            .publish(
                OperationPhase::Running,
                EventKind::Progress(Progress::Capturing(CapturePercent::new(42).unwrap()))
            )
            .unwrap(),
        2
    );
    assert_eq!(stream.last_sequence(), 2);
}

#[test]
fn terminal_event_is_last_even_when_history_has_wrapped() {
    let mut stream = stream();
    for _ in 0..HISTORY_MAX {
        stream
            .publish(
                OperationPhase::Running,
                EventKind::Progress(Progress::Working),
            )
            .unwrap();
    }
    let terminal = stream
        .publish(OperationPhase::Terminal, EventKind::Terminal)
        .unwrap();
    assert_eq!(
        stream.publish(OperationPhase::Terminal, EventKind::Terminal),
        Err(EventError::AlreadyTerminal)
    );
    assert_eq!(
        stream.publish(
            OperationPhase::Running,
            EventKind::Progress(Progress::Working)
        ),
        Err(EventError::AlreadyTerminal)
    );
    assert_eq!(stream.last_sequence(), terminal);
}

#[test]
fn retained_terminal_requires_completed_verification() {
    let mut stream = stream();
    stream
        .publish(OperationPhase::Terminal, EventKind::Terminal)
        .unwrap();
    let mut verification = state();
    assert_eq!(
        stream.replay_from(0, &verification),
        Err(SnapshotError::IncompleteVerification)
    );
    verification.common.phase = OperationPhase::Terminal;
    assert_eq!(
        stream.replay_from(0, &verification),
        Err(SnapshotError::IncompleteVerification)
    );
    verification.common.terminal = Some(TerminalOutcome::NoSample);
    assert!(
        matches!(stream.replay_from(0, &verification), Ok(Replay::Events(events)) if events.len() == 1)
    );
}

#[test]
fn replay_reports_gap_instead_of_hiding_it() {
    let mut stream = stream();
    for _ in 0..(HISTORY_MAX + 5) {
        stream
            .publish(
                OperationPhase::Running,
                EventKind::Progress(Progress::Working),
            )
            .unwrap();
    }
    let state = state();
    let replay = stream.replay_from(0, &state).unwrap();
    assert_eq!(
        replay,
        Replay::Gap {
            after_sequence: 0,
            last_sequence: 133,
            snapshot: OperationSnapshot {
                operation: state.common.context.operation_id,
                phase: OperationPhase::New,
                last_sequence: 133,
                terminal: None
            },
        }
    );
}

#[test]
fn in_range_replay_has_no_gap() {
    let mut stream = stream();
    stream
        .publish(OperationPhase::Running, EventKind::StateChanged)
        .unwrap();
    stream
        .publish(OperationPhase::Terminal, EventKind::Terminal)
        .unwrap();
    let mut completed = state();
    completed.common.phase = OperationPhase::Terminal;
    completed.common.terminal = Some(TerminalOutcome::NoSample);
    let replay = stream.replay_from(1, &completed).unwrap();
    let Replay::Events(events) = replay else {
        panic!("не должно быть пропуска")
    };
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, EventKind::Terminal));
    assert_eq!(events[0].phase, OperationPhase::Terminal);
}

#[test]
fn replay_preserves_phase_at_each_event() {
    let mut stream = stream();
    stream
        .publish(OperationPhase::WaitingLease, EventKind::StateChanged)
        .unwrap();
    stream
        .publish(
            OperationPhase::Running,
            EventKind::Progress(Progress::Working),
        )
        .unwrap();
    stream
        .publish(OperationPhase::Finalizing, EventKind::StateChanged)
        .unwrap();
    let Replay::Events(events) = stream.replay_from(0, &state()).unwrap() else {
        panic!("не должно быть пропуска")
    };
    assert_eq!(
        events.iter().map(|event| event.phase).collect::<Vec<_>>(),
        vec![
            OperationPhase::WaitingLease,
            OperationPhase::Running,
            OperationPhase::Finalizing
        ]
    );
}

#[test]
fn replay_after_maximum_sequence_is_empty_without_overflow() {
    let mut stream = stream();
    stream
        .publish(OperationPhase::New, EventKind::StateChanged)
        .unwrap();
    assert_eq!(
        stream.replay_from(u64::MAX, &state()),
        Ok(Replay::Events(Vec::new()))
    );
}

#[test]
fn exhausted_sequence_does_not_wrap_or_change_history() {
    let mut stream = EventStream {
        operation: state().common.context.operation_id,
        last: u64::MAX - 1,
        history: Vec::new(),
    };
    assert_eq!(
        stream.publish(OperationPhase::New, EventKind::StateChanged),
        Ok(u64::MAX)
    );
    assert_eq!(
        stream.publish(OperationPhase::Terminal, EventKind::Terminal),
        Err(EventError::SequenceExhausted)
    );
    assert_eq!(stream.last_sequence(), u64::MAX);
    assert_eq!(stream.history.len(), 1);
}

#[test]
fn snapshot_does_not_publish_unconfirmed_or_foreign_results() {
    use carosign_core::{
        enrollment::EnrollmentLabel, ids::MutationId, operation::mutation::EnrollRequest,
    };
    use carosign_engine::{PlannedEffect, TransitionIndex};
    let mut stream = stream();
    for _ in 0..=HISTORY_MAX {
        stream
            .publish(OperationPhase::New, EventKind::StateChanged)
            .unwrap();
    }
    let mut verification = state();
    verification.common.context.operation_id = OperationId::from_bytes([9; 16]).unwrap();
    assert_eq!(
        stream.replay_from(0, &verification),
        Err(SnapshotError::WrongOperation)
    );
    verification = state();
    verification.common.phase = OperationPhase::Terminal;
    verification.common.terminal = Some(TerminalOutcome::FactorAccepted);
    verification.common.pending_effects.push(PlannedEffect::at(
        EffectSpec::RecordTerminal,
        TransitionIndex::new(1),
        0,
    ));
    assert_eq!(
        stream.replay_from(0, &verification),
        Err(SnapshotError::IncompleteVerification)
    );
    verification.common.pending_effects.clear();
    assert!(matches!(
        stream.replay_from(0, &verification),
        Ok(Replay::Gap {
            snapshot: OperationSnapshot {
                terminal: Some(TerminalOutcome::FactorAccepted),
                ..
            },
            ..
        })
    ));
    let context = state().common.context;
    let mut mutation = OperationState::enrollment(
        context.clone(),
        EnrollRequest {
            subject: context.subject,
            authenticator: context.authenticator.id,
            target: EnrollmentLabel::new("палец").unwrap(),
            replace: None,
        },
        MutationId::from_bytes([7; 16]).unwrap(),
    );
    mutation.common.phase = OperationPhase::Terminal;
    mutation.common.terminal = Some(TerminalOutcome::MutationCommitted);
    assert_eq!(
        stream.replay_from(0, &mutation),
        Err(SnapshotError::MutationOutcomeNeedsStorage)
    );
    let mut recent = EventStream::new(context.operation_id);
    recent
        .publish(OperationPhase::Running, EventKind::StateChanged)
        .unwrap();
    recent
        .publish(OperationPhase::Terminal, EventKind::Terminal)
        .unwrap();
    assert!(matches!(
        recent.replay_from(0, &mutation),
        Err(SnapshotError::MutationOutcomeNeedsStorage)
    ));
    assert!(matches!(
        recent.replay_with_mutation_outcome(0, &mutation, TerminalOutcome::NoChange),
        Ok(Replay::Events(_))
    ));
    assert!(
        matches!(recent.replay_from(2, &mutation), Ok(Replay::Events(events)) if events.is_empty())
    );
}
