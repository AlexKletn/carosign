use carosign_core::ids::{MutationId, OperationId};

use super::{OutcomeReadError, storage_failure::map_error};

/// Receives a stable result after fresh verification of the right to observe the operation.
///
/// The caller is responsible for the trusted matching of the operation and the mutation; the
/// repository itself reconfirms both identifiers. This is not a public IPC method.
pub async fn authorized_completed_outcome(
    authority: &dyn carosign_runtime::ports::PolicyAuthority,
    caller: &carosign_runtime::ports::CallerContext,
    store: &dyn carosign_persistence::store::MetadataStore,
    mutation: MutationId,
    operation: OperationId,
) -> Result<carosign_persistence::model::MutationTerminalOutcome, OutcomeReadError> {
    let grant = carosign_runtime::operation::observer::authorize(authority, caller, operation)
        .await
        .map_err(OutcomeReadError::Authority)?;
    let outcome =
        carosign_persistence::recovery::read_completed_mutation_v3(store, mutation, operation)
            .map_err(|error| OutcomeReadError::Storage(map_error(error)))?;
    authority
        .revalidate_caller(
            caller,
            &grant,
            carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
        )
        .await
        .map_err(OutcomeReadError::Authority)?;
    Ok(outcome)
}

/// Looking for a sustainable outcome of the operation after the new authorization of the
/// observer.
///
/// The transaction number does not entitle: the source of the policy is called before reading
/// and rechecked before returning.
pub async fn authorized_completed_outcome_by_operation(
    authority: &dyn carosign_runtime::ports::PolicyAuthority,
    caller: &carosign_runtime::ports::CallerContext,
    store: &dyn carosign_persistence::store::MetadataStore,
    operation: OperationId,
) -> Result<Option<carosign_persistence::model::MutationTerminalOutcome>, OutcomeReadError> {
    let grant = carosign_runtime::operation::observer::authorize(authority, caller, operation)
        .await
        .map_err(OutcomeReadError::Authority)?;
    let outcome = carosign_persistence::recovery::read_completed_by_operation_v3(store, operation)
        .map_err(|error| OutcomeReadError::Storage(map_error(error)))?;
    authority
        .revalidate_caller(
            caller,
            &grant,
            carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
        )
        .await
        .map_err(OutcomeReadError::Authority)?;
    Ok(outcome)
}
