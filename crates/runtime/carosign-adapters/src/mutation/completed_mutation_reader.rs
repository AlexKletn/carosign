use carosign_core::ids::MutationId;
use carosign_core::ids::OperationId;

/// Port of Sustainable Results for Observer Subscription
pub struct CompletedMutationReader<'a> {
    store: &'a dyn carosign_persistence::store::MetadataStore,
}

impl<'a> CompletedMutationReader<'a> {
    /// Creates a port on top of the selected v3 storage.
    pub fn new(store: &'a dyn carosign_persistence::store::MetadataStore) -> Self {
        Self { store }
    }
}

impl carosign_runtime::operation::observer::TerminalOutcomePort for CompletedMutationReader<'_> {
    fn read(
        &self,
        operation: OperationId,
        mutation: MutationId,
    ) -> carosign_runtime::BoxFuture<'_, Result<carosign_engine::TerminalOutcome, ()>> {
        let result = carosign_persistence::recovery::read_completed_mutation_v3(
            self.store, mutation, operation,
        )
        .map(|outcome| match outcome {
            carosign_persistence::model::MutationTerminalOutcome::Committed => {
                carosign_engine::TerminalOutcome::MutationCommitted
            }
            carosign_persistence::model::MutationTerminalOutcome::NoChange => {
                carosign_engine::TerminalOutcome::NoChange
            }
        })
        .map_err(|_| ());
        Box::pin(async move { result })
    }
}
