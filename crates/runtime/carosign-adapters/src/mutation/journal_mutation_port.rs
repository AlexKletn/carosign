use super::{MutationIntent, StorageFailure, storage_failure::map_error};

use carosign_core::ids::{CommandId, OperationId};
use carosign_persistence::journal::{MutationRepository, PrepareMutation};
use carosign_persistence::model::MutationClosure;
use carosign_runtime::ports::{DurableMarker, MutationPort};

/// Magazine adapter.
///
/// The weapons are placed in a separate port and are not carried out here.
pub struct JournalMutationPort<'a, R: MutationRepository> {
    repository: &'a R,
    intent: MutationIntent,
    /// What's left to do at closing.
    _closure: MutationClosure,
}

impl<'a, R: MutationRepository> core::fmt::Debug for JournalMutationPort<'a, R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("JournalMutationPort(..)")
    }
}

impl<'a, R: MutationRepository> JournalMutationPort<'a, R> {
    /// It assembles an adapter for one operation.
    pub fn new(
        repository: &'a R,
        intent: MutationIntent,
        _command: CommandId,
        closure: MutationClosure,
    ) -> Self {
        Self {
            repository,
            intent,
            _closure: closure,
        }
    }

    /// Preparing an intention.
    ///
    /// ⚠️ Separate method, not port: the engine asks for a “write of intent,” but what to
    /// write, the owner of the operation knows, not he.
    pub fn prepare(&self) -> Result<(), StorageFailure> {
        self.repository
            .prepare(PrepareMutation {
                mutation: self.intent.mutation,
                operation: self.intent.operation,
                subject: self.intent.subject,
                authenticator: self.intent.authenticator,
                expected_generation: self.intent.expected_generation,
                enroll: self.intent.enroll,
                plan: self.intent.plan.clone(),
            })
            .map(|_| ())
            .map_err(map_error)
    }
}

impl<R: MutationRepository> MutationPort for JournalMutationPort<'_, R> {
    fn persist_intent(
        &self,
        operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        let durable = operation == self.intent.operation && self.prepare().is_ok();
        Box::pin(async move { DurableMarker { durable } })
    }

    fn commit_registry(
        &self,
        _operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        // There is no verified action and transaction confirmation.
        Box::pin(async move { DurableMarker { durable: false } })
    }

    fn commit_reconciliation(
        &self,
        _operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        Box::pin(async move { DurableMarker { durable: false } })
    }

    fn mark_uncertain(
        &self,
        operation: OperationId,
    ) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        let durable = operation == self.intent.operation
            && self.repository.mark_uncertain(self.intent.mutation).is_ok();
        Box::pin(async move { DurableMarker { durable } })
    }

    fn finalize(&self, _operation: OperationId) -> carosign_runtime::BoxFuture<'_, DurableMarker> {
        // The current port does not transmit the confirmed stop and the verified outcome, and
        // the record of uncertainty does not constitute the completion of the operation.
        Box::pin(async move { DurableMarker { durable: false } })
    }
}
