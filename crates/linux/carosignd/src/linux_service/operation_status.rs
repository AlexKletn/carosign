use carosign_core::ids::OperationId;
use carosign_engine::{OperationPhase, TerminalOutcome};
use carosign_persistence::{
    model::{MutationPhase, MutationTerminalOutcome},
    store::MetadataStore,
};
use carosign_protocol::carosign::client::v1 as wire;
use carosign_runtime::{operation::events::OperationSnapshot, ports::AuthorityRefusal};

use crate::{linux_policy::LinuxPolicy, operations::LiveOperationStatus};

use super::operation_directory::OperationDirectory;

pub(super) struct OperationStatus<'a> {
    pub(super) observer_limit: usize,
    pub(super) directory: &'a OperationDirectory,
    pub(super) policy: &'a LinuxPolicy,
    pub(super) store: &'a dyn MetadataStore,
}

impl OperationStatus<'_> {
    pub(super) fn read(
        &self,
        uid: u32,
        operation: OperationId,
    ) -> Result<wire::OperationSnapshot, AuthorityRefusal> {
        self.policy.authorize_observation(uid, None)?;
        let live = self.directory.find(operation);
        if let Some(live) = &live {
            self.policy.authorize_observation(uid, Some(live.subject))?;
            if live.phase != OperationPhase::Terminal || live.verification_outcome.is_some() {
                let snapshot = live_snapshot(operation, live);
                self.policy.authorize_observation(uid, Some(live.subject))?;
                return Ok(snapshot);
            }
            if !live.is_mutation {
                return Err(AuthorityRefusal::Unavailable);
            }
        }

        let record = carosign_persistence::mutation_store::read_mutation_by_operation_v4(
            self.store, operation,
        )
        .map_err(|_| AuthorityRefusal::Unavailable)?
        .ok_or(AuthorityRefusal::Denied)?;
        self.policy
            .authorize_observation(uid, Some(record.subject))?;
        if live
            .as_ref()
            .is_some_and(|entry| entry.subject != record.subject)
        {
            return Err(AuthorityRefusal::Unavailable);
        }
        let last_sequence = live.map_or(0, |entry| entry.last_sequence);
        if record.phase == MutationPhase::Completed {
            let terminal = match record.terminal_outcome {
                Some(MutationTerminalOutcome::Committed) => TerminalOutcome::MutationCommitted,
                Some(MutationTerminalOutcome::NoChange) => TerminalOutcome::NoChange,
                None => return Err(AuthorityRefusal::Unavailable),
            };
            return crate::answer::completed_mutation_snapshot(&OperationSnapshot {
                operation,
                phase: OperationPhase::Terminal,
                last_sequence,
                terminal: Some(terminal),
            })
            .ok_or(AuthorityRefusal::Unavailable);
        }
        Ok(wire::OperationSnapshot {
            operation_id: operation.as_bytes().to_vec(),
            phase: wire::OperationPhase::OpNeedsReconciliation as i32,
            last_event_sequence: last_sequence,
            terminal: None,
        })
    }
}

fn live_snapshot(operation: OperationId, live: &LiveOperationStatus) -> wire::OperationSnapshot {
    let phase = crate::answer::public_phase(live.phase);
    let terminal = live
        .verification_outcome
        .map(crate::answer::verification_terminal);
    wire::OperationSnapshot {
        operation_id: operation.as_bytes().to_vec(),
        phase: phase as i32,
        last_event_sequence: live.last_sequence,
        terminal,
    }
}
