use super::{OperationId, ReplayEncodingError, client};

/// Converts a confirmed snapshot of the completed change into a wire.
///
/// The caller is obliged to check the right of observation and get an outcome from the
/// journal.
/// This converter does not confirm either authority or sustainability.
pub fn completed_mutation_snapshot(
    snapshot: &carosign_runtime::operation::events::OperationSnapshot,
) -> Option<client::OperationSnapshot> {
    use carosign_engine::{OperationPhase, TerminalOutcome};
    if snapshot.phase != OperationPhase::Terminal {
        return None;
    }
    let result = match snapshot.terminal? {
        TerminalOutcome::MutationCommitted => client::MutationResult::MutationCommitted,
        TerminalOutcome::NoChange => client::MutationResult::MutationNoChange,
        _ => return None,
    };
    Some(client::OperationSnapshot {
        operation_id: snapshot.operation.as_bytes().to_vec(),
        phase: client::OperationPhase::OpTerminal as i32,
        last_event_sequence: snapshot.last_sequence,
        terminal: Some(client::TerminalResult {
            kind: Some(client::terminal_result::Kind::Mutation(
                client::MutationTerminal {
                    result: result as i32,
                    enrollment: None,
                    mutation_id: Vec::new(),
                    error: None,
                },
            )),
        }),
    })
}

/// Converts the saved event of the completed mutation into a client frame.
///
/// The caller transmits the outcome only after reading the log and authorization.
/// The skips and pictures are processed separately.
pub fn completed_mutation_event(
    operation: OperationId,
    event: &carosign_runtime::operation::events::Event,
    outcome: carosign_engine::TerminalOutcome,
) -> Option<client::ClientFrame> {
    use carosign_engine::{OperationPhase as P, TerminalOutcome as O};
    use carosign_runtime::operation::events::EventKind as K;
    let phase = super::public_phase(event.phase);
    if event.sequence == 0 || !matches!(outcome, O::MutationCommitted | O::NoChange) {
        return None;
    }
    let (kind, percent, feedback, terminal) = match &event.kind {
        K::StateChanged => (client::EventKind::EventStateChanged, 0, None, None),
        K::Progress(progress) => {
            let (kind, percent, feedback) = super::capture_feedback::encode_progress(*progress);
            (kind, percent, feedback, None)
        }
        K::Terminal if event.phase == P::Terminal => {
            let snapshot = carosign_runtime::operation::events::OperationSnapshot {
                operation,
                phase: P::Terminal,
                last_sequence: event.sequence,
                terminal: Some(outcome),
            };
            (
                client::EventKind::EventTerminal,
                0,
                None,
                completed_mutation_snapshot(&snapshot)?.terminal,
            )
        }
        K::Terminal => return None,
    };
    Some(client::ClientFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_MAX,
        request_id: Vec::new(),
        operation_id: operation.as_bytes().to_vec(),
        event_sequence: event.sequence,
        payload: Some(client::client_frame::Payload::OperationEvent(
            client::OperationEvent {
                capture_state: None,
                feedback,
                operation_id: operation.as_bytes().to_vec(),
                sequence: event.sequence,
                kind: kind as i32,
                phase: phase as i32,
                percent,
                gap_after_sequence: 0,
                snapshot: None,
                terminal,
            },
        )),
    })
}

/// Transforms the sequence of events of a complete mutation.
///
/// Footage is created only after checking the entire history: partial rendition in case of
/// contradiction is prohibited.
pub fn completed_mutation_replay_frames(
    replay: &carosign_adapters::mutation::AuthorizedMutationReplay,
) -> Result<Vec<client::ClientFrame>, ReplayEncodingError> {
    use carosign_runtime::operation::events::{EventKind, Replay};
    let Replay::Events(events) = &replay.replay else {
        return Err(ReplayEncodingError::GapNeedsSequenceContract);
    };
    let mut frames = Vec::with_capacity(events.len());
    let mut previous = None;
    let mut terminal_seen = false;
    for event in events {
        if terminal_seen
            || previous.is_some_and(|number: u64| number.checked_add(1) != Some(event.sequence))
        {
            return Err(ReplayEncodingError::InvalidHistory);
        }
        terminal_seen = event.kind == EventKind::Terminal;
        previous = Some(event.sequence);
        frames.push(
            completed_mutation_event(replay.operation, event, replay.outcome)
                .ok_or(ReplayEncodingError::InvalidHistory)?,
        );
    }
    Ok(frames)
}
