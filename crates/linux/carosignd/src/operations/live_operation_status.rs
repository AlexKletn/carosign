use carosign_core::{identity::SubjectRef, ids::OperationId};
use carosign_engine::{OperationPhase, TerminalOutcome};

use super::{Entry, OperationRegistry};

#[derive(Clone)]
pub(crate) struct LiveOperationStatus {
    pub(crate) is_mutation: bool,
    pub(crate) subject: SubjectRef,
    pub(crate) phase: OperationPhase,
    pub(crate) last_sequence: u64,
    pub(crate) verification_outcome: Option<TerminalOutcome>,
}

impl OperationRegistry {
    pub(crate) fn live_status(&self, operation: OperationId) -> Option<LiveOperationStatus> {
        let entries = self.entries.lock().ok()?;
        if let Some(retired) = entries.retired.get(&operation) {
            return Some(retired.status.clone());
        }
        entries.get(&operation).map(LiveOperationStatus::from_entry)
    }
}

impl LiveOperationStatus {
    pub(super) fn from_entry(entry: &Entry) -> Self {
        let hub = entry
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let stream = hub.stream();
        let published = stream
            .latest()
            .map_or(OperationPhase::New, |event| event.phase);
        let phase = entry.operation.as_ref().map_or(published, |owned| {
            let state = owned.actor.state();
            if state.is_mutation() && published != OperationPhase::Terminal {
                state.common.phase
            } else {
                published
            }
        });
        let verification_outcome = entry.operation.as_ref().and_then(|owned| {
            let state = owned.actor.state();
            match state.common.terminal {
                Some(
                    outcome @ (TerminalOutcome::NotMatched
                    | TerminalOutcome::NoSample
                    | TerminalOutcome::Cancelled
                    | TerminalOutcome::Unavailable
                    | TerminalOutcome::Failed),
                ) if !state.is_mutation() && phase == OperationPhase::Terminal => Some(outcome),
                _ => None,
            }
        });
        Self {
            is_mutation: entry.is_mutation,
            subject: entry.subject,
            phase,
            last_sequence: stream.last_sequence(),
            verification_outcome,
        }
    }
}
