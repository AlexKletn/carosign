use super::{
    AuthorizationBoundary, EffectRunner, EventNotification, Observation, ObservedExecution,
    OperationActor, PolicyAuthority, RunOutcome, TerminalPublication, TerminalPublicationError,
    ValidatedGrant, grant_matches_operation, run_inner,
};

/// Publishes the completed mutation after reading the journal and checking the owner.
///
/// The result is from a steady source, not from the actor's state, and the individual review
/// of each observer remains the responsibility of the subscription.
pub async fn publish_observed_mutation_terminal(
    actor: &OperationActor,
    hub: &mut carosign_runtime::operation::hub::EventHub,
    authority: &dyn PolicyAuthority,
    grant: &ValidatedGrant,
    storage: &dyn carosign_runtime::operation::observer::TerminalOutcomePort,
) -> Result<TerminalPublication, TerminalPublicationError> {
    let state = actor.state();
    if !state.common.phase.is_terminal()
        || !grant_matches_operation(actor, grant)
        || hub.operation() != state.common.context.operation_id
    {
        return Err(TerminalPublicationError::ContextMismatch);
    }
    if state.mutation().is_none() {
        return Err(TerminalPublicationError::ContextMismatch);
    }
    let owner_before = authority
        .revalidate(grant, AuthorizationBoundary::BeforeDelivery)
        .await;
    let (outcome, disconnected) = hub
        .publish_confirmed_mutation_terminal(state, storage)
        .await
        .map_err(|error| match error {
            carosign_runtime::operation::hub::TerminalPublishError::Unavailable => {
                TerminalPublicationError::StorageUnavailable
            }
            other => TerminalPublicationError::Event(other),
        })?;
    let owner_outcome = match owner_before {
        Ok(()) => authority
            .revalidate(grant, AuthorizationBoundary::BeforeDelivery)
            .await
            .map(|()| outcome),
        Err(error) => Err(error),
    };
    Ok(TerminalPublication {
        owner_outcome,
        disconnected,
    })
}

/// Conducts an operation with the publication of phases and a separate notification of
/// overflow.
pub async fn run_observed(
    actor: &mut OperationActor,
    runner: &mut EffectRunner<'_>,
    authority: &dyn PolicyAuthority,
    grant: &ValidatedGrant,
    observe: &mut Observation<'_>,
    hub: &mut carosign_runtime::operation::hub::EventHub,
    notify: &mut EventNotification<'_>,
) -> RunOutcome {
    run_inner(
        actor,
        runner,
        authority,
        grant,
        observe,
        Some(ObservedExecution {
            hub,
            notify,
            terminal_storage: None,
        }),
    )
    .await
}

/// Operates with a hub and reads a steady mutation outcome.
pub async fn run_observed_with_storage(
    actor: &mut OperationActor,
    runner: &mut EffectRunner<'_>,
    authority: &dyn PolicyAuthority,
    grant: &ValidatedGrant,
    observe: &mut Observation<'_>,
    events: ObservedExecution<'_>,
) -> RunOutcome {
    run_inner(actor, runner, authority, grant, observe, Some(events)).await
}
