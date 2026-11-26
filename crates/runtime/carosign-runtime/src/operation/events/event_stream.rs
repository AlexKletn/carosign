use super::{Event, EventError, EventKind, HISTORY_MAX, OperationSnapshot, Replay, SnapshotError};
use carosign_core::ids::OperationId;
use carosign_engine::{EffectSpec, OperationPhase, OperationState, TerminalOutcome};

/// A stream of events in one operation.
#[derive(Debug)]
pub struct EventStream {
    operation: OperationId,
    last: u64,
    history: Vec<Event>,
}

impl EventStream {
    /// Empty stream; the first event is number 1.
    pub fn new(operation: OperationId) -> Self {
        Self {
            operation,
            last: 0,
            history: Vec::new(),
        }
    }

    /// Adds an event.
    pub fn publish(&mut self, phase: OperationPhase, kind: EventKind) -> Result<u64, EventError> {
        if self
            .history
            .last()
            .is_some_and(|event| event.kind == EventKind::Terminal)
        {
            return Err(EventError::AlreadyTerminal);
        }
        let sequence = self
            .last
            .checked_add(1)
            .ok_or(EventError::SequenceExhausted)?;
        self.last = sequence;
        self.history.push(Event {
            sequence,
            phase,
            kind,
        });
        if self.history.len() > HISTORY_MAX {
            self.history.remove(0);
        }
        Ok(sequence)
    }

    /// What number do we know?
    ///
    /// When skipping, it creates a picture from the same operation state.
    /// Old events do not return after the actual picture: they could roll back the observer’s
    /// state.
    /// This is not a test of the powers of the observer.
    pub fn replay_from(
        &self,
        after_sequence: u64,
        state: &OperationState,
    ) -> Result<Replay, SnapshotError> {
        self.replay_inner(after_sequence, state, None)
    }

    /// Builds a repeat with a verified sustainable change outcome.
    ///
    /// The caller has to get an exit from a trusted repository and separately verify the
    /// right of surveillance, a method that does not issue permissions.
    pub fn replay_with_mutation_outcome(
        &self,
        after_sequence: u64,
        state: &OperationState,
        outcome: TerminalOutcome,
    ) -> Result<Replay, SnapshotError> {
        if !state.is_mutation()
            || !state.common.phase.is_terminal()
            || !matches!(
                outcome,
                TerminalOutcome::MutationCommitted | TerminalOutcome::NoChange
            )
        {
            return Err(SnapshotError::MutationOutcomeNeedsStorage);
        }
        self.replay_inner(after_sequence, state, Some(outcome))
    }

    fn replay_inner(
        &self,
        after_sequence: u64,
        state: &OperationState,
        stored_outcome: Option<TerminalOutcome>,
    ) -> Result<Replay, SnapshotError> {
        if state.common.context.operation_id != self.operation {
            return Err(SnapshotError::WrongOperation);
        }
        let first_known = self.history.first().map(|event| event.sequence);
        if first_known.is_some_and(|first| after_sequence < first.saturating_sub(1)) {
            let terminal = if state.common.phase.is_terminal() {
                if state.is_mutation() {
                    Some(stored_outcome.ok_or(SnapshotError::MutationOutcomeNeedsStorage)?)
                } else {
                    if state.common.pending_effects.iter().any(|effect| {
                        matches!(
                            effect.spec,
                            EffectSpec::RecordTerminal | EffectSpec::DeliverOutcome
                        )
                    }) {
                        return Err(SnapshotError::IncompleteVerification);
                    }
                    match state.common.terminal {
                        Some(
                            outcome @ (TerminalOutcome::FactorAccepted
                            | TerminalOutcome::NotMatched
                            | TerminalOutcome::NoSample
                            | TerminalOutcome::Cancelled
                            | TerminalOutcome::Unavailable
                            | TerminalOutcome::Failed),
                        ) => Some(outcome),
                        _ => return Err(SnapshotError::IncompleteVerification),
                    }
                }
            } else {
                None
            };
            return Ok(Replay::Gap {
                after_sequence,
                last_sequence: self.last,
                snapshot: OperationSnapshot {
                    operation: self.operation,
                    phase: state.common.phase,
                    last_sequence: self.last,
                    terminal,
                },
            });
        }
        let events: Vec<_> = self
            .history
            .iter()
            .filter(|event| event.sequence > after_sequence)
            .cloned()
            .collect();
        if state.is_mutation()
            && stored_outcome.is_none()
            && events.iter().any(|event| event.kind == EventKind::Terminal)
        {
            return Err(SnapshotError::MutationOutcomeNeedsStorage);
        }
        if !state.is_mutation()
            && events.iter().any(|event| event.kind == EventKind::Terminal)
            && (!state.common.phase.is_terminal()
                || state.common.pending_effects.iter().any(|effect| {
                    matches!(
                        effect.spec,
                        EffectSpec::RecordTerminal | EffectSpec::DeliverOutcome
                    )
                })
                || !matches!(
                    state.common.terminal,
                    Some(
                        TerminalOutcome::FactorAccepted
                            | TerminalOutcome::NotMatched
                            | TerminalOutcome::NoSample
                            | TerminalOutcome::Cancelled
                            | TerminalOutcome::Unavailable
                            | TerminalOutcome::Failed
                    )
                ))
        {
            return Err(SnapshotError::IncompleteVerification);
        }
        Ok(Replay::Events(events))
    }

    pub fn retained_after(&self, sequence: u64) -> Option<Vec<Event>> {
        if self
            .history
            .first()
            .is_some_and(|first| sequence < first.sequence.saturating_sub(1))
        {
            return None;
        }
        Some(
            self.history
                .iter()
                .filter(|event| event.sequence > sequence)
                .cloned()
                .collect(),
        )
    }

    pub fn latest(&self) -> Option<&Event> {
        self.history.last()
    }

    /// Last number issued.
    pub fn last_sequence(&self) -> u64 {
        self.last
    }
}

#[path = "event_stream.tests.rs"]
#[cfg(test)]
mod tests;
