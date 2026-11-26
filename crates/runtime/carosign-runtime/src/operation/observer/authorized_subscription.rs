use super::{SubscriptionError, TerminalOutcomePort};
use crate::ports::{AuthorityRefusal, CallerContext, PolicyAuthority};
use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::ids::{MutationId, OperationId};

/// Requests the right to observe a specific operation.
///
/// The transaction number itself does not give rights. When you reconnect, the caller
/// receives a new resolution; the source failure is not replaced by the previous issuance.
pub async fn authorize(
    authority: &dyn PolicyAuthority,
    caller: &CallerContext,
    operation: OperationId,
) -> Result<ValidatedGrant, AuthorityRefusal> {
    let scope = GrantScope::Observe(operation);
    let grant = authority.authorize(caller, scope.clone()).await?;
    if grant.scope() != &scope {
        return Err(AuthorityRefusal::Denied);
    }
    Ok(grant)
}

/// Subscription with permission obtained from the source of the policy.
pub struct AuthorizedSubscription {
    operation: OperationId,
    id: u32,
    generation: u64,
    hub_identity: std::sync::Arc<()>,
    grant: ValidatedGrant,
    caller: CallerContext,
}

impl AuthorizedSubscription {
    /// Checks the authority and connects the queue to the current position.
    ///
    /// The caller must not hold global locks during the call.
    pub async fn attach(
        authority: &dyn PolicyAuthority,
        caller: &CallerContext,
        hub: &mut super::super::hub::EventHub,
        id: u32,
        after_sequence: u64,
    ) -> Result<Self, SubscriptionError> {
        let operation = hub.operation();
        let grant = authorize(authority, caller, operation)
            .await
            .map_err(SubscriptionError::Authority)?;
        authority
            .revalidate_caller(
                caller,
                &grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
            .map_err(SubscriptionError::Authority)?;
        hub.attach(id, after_sequence)
            .map_err(SubscriptionError::Attach)?;
        let generation = hub.generation(id).ok_or(SubscriptionError::Detached)?;
        Ok(Self {
            operation,
            id,
            generation,
            hub_identity: hub.identity(),
            grant,
            caller: caller.clone(),
        })
    }

    /// Recheck the right before taking another non-terminal event.
    ///
    /// The terminal marker stays in line until a separate, authorised sustainable outcome
    /// processing is completed. Global locks cannot be held.
    pub async fn next(
        &self,
        authority: &dyn PolicyAuthority,
        hub: &mut super::super::hub::EventHub,
    ) -> Result<Option<super::super::events::Event>, SubscriptionError> {
        if hub.operation() != self.operation
            || !hub.matches_identity(&self.hub_identity)
            || hub.generation(self.id) != Some(self.generation)
        {
            return Err(SubscriptionError::Detached);
        }
        if let Err(error) = authority
            .revalidate_caller(
                &self.caller,
                &self.grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
        {
            hub.detach(self.id);
            return Err(SubscriptionError::Authority(error));
        }
        if hub
            .peek(self.id)
            .is_some_and(|event| event.kind == super::super::events::EventKind::Terminal)
        {
            return Err(SubscriptionError::TerminalNeedsStorage);
        }
        Ok(hub.pop(self.id))
    }

    /// Provides a terminal marker only together with a stable outcome.
    ///
    /// The port and the source of the policy must be trusted. If the reading error, the token
    /// remains in the queue; when the subscription is withdrawn, the subscription is deleted.
    pub async fn next_mutation_terminal(
        &self,
        authority: &dyn PolicyAuthority,
        storage: &dyn TerminalOutcomePort,
        mutation: MutationId,
        hub: &mut super::super::hub::EventHub,
    ) -> Result<
        Option<(
            super::super::events::Event,
            carosign_engine::TerminalOutcome,
        )>,
        SubscriptionError,
    > {
        if hub.operation() != self.operation
            || !hub.matches_identity(&self.hub_identity)
            || hub.generation(self.id) != Some(self.generation)
        {
            return Err(SubscriptionError::Detached);
        }
        let Some(event) = hub.peek(self.id) else {
            return Ok(None);
        };
        if event.kind != super::super::events::EventKind::Terminal || !event.phase.is_terminal() {
            return Err(SubscriptionError::TerminalNeedsStorage);
        }
        if let Err(error) = authority
            .revalidate_caller(
                &self.caller,
                &self.grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
        {
            hub.detach(self.id);
            return Err(SubscriptionError::Authority(error));
        }
        let outcome = storage
            .read(self.operation, mutation)
            .await
            .map_err(|_| SubscriptionError::TerminalUnavailable)?;
        if !matches!(
            outcome,
            carosign_engine::TerminalOutcome::MutationCommitted
                | carosign_engine::TerminalOutcome::NoChange
        ) {
            return Err(SubscriptionError::TerminalUnavailable);
        }
        if let Err(error) = authority
            .revalidate_caller(
                &self.caller,
                &self.grant,
                carosign_core::authorization::AuthorizationBoundary::BeforeDelivery,
            )
            .await
        {
            hub.detach(self.id);
            return Err(SubscriptionError::Authority(error));
        }
        let event = hub.pop(self.id).ok_or(SubscriptionError::Detached)?;
        Ok(Some((event, outcome)))
    }

    /// Unsubscribes the observer without canceling the operation.
    pub fn detach(self, hub: &mut super::super::hub::EventHub) -> bool {
        hub.operation() == self.operation
            && hub.matches_identity(&self.hub_identity)
            && hub.generation(self.id) == Some(self.generation)
            && hub.detach(self.id)
    }
}

#[cfg(test)]
#[path = "authorized_subscription.tests.rs"]
mod tests;
