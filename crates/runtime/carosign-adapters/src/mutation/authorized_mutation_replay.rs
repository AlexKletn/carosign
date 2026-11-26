use super::{OutcomeReplayError, authorized_completed_outcome};

use carosign_core::ids::OperationId;

/// Repeat and endurance to fill the terminal event.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AuthorizedMutationReplay {
    /// The operation to which the events and the verified outcome relate.
    pub operation: OperationId,
    /// Events or a snapshot after a skip.
    pub replay: carosign_runtime::operation::events::Replay,
    /// The result is from the magazine, not from the event ring.
    pub outcome: carosign_engine::TerminalOutcome,
}

/// Returns a repeat with a stable result after the authorization of the observer.
pub async fn authorized_completed_replay(
    authority: &dyn carosign_runtime::ports::PolicyAuthority,
    caller: &carosign_runtime::ports::CallerContext,
    store: &dyn carosign_persistence::store::MetadataStore,
    stream: &carosign_runtime::operation::events::EventStream,
    state: &carosign_engine::OperationState,
    after_sequence: u64,
) -> Result<AuthorizedMutationReplay, OutcomeReplayError> {
    let mutation = state
        .mutation()
        .ok_or(OutcomeReplayError::Snapshot(
            carosign_runtime::operation::events::SnapshotError::MutationOutcomeNeedsStorage,
        ))?
        .mutation;
    let outcome = authorized_completed_outcome(
        authority,
        caller,
        store,
        mutation,
        state.common.context.operation_id,
    )
    .await
    .map_err(OutcomeReplayError::Outcome)?;
    let outcome = match outcome {
        carosign_persistence::model::MutationTerminalOutcome::Committed => {
            carosign_engine::TerminalOutcome::MutationCommitted
        }
        carosign_persistence::model::MutationTerminalOutcome::NoChange => {
            carosign_engine::TerminalOutcome::NoChange
        }
    };
    let replay = stream
        .replay_with_mutation_outcome(after_sequence, state, outcome)
        .map_err(OutcomeReplayError::Snapshot)?;
    Ok(AuthorizedMutationReplay {
        operation: state.common.context.operation_id,
        replay,
        outcome,
    })
}
