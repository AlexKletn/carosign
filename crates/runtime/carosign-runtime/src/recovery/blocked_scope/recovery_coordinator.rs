use super::Entry;
use super::RECOVERY_SCOPES_MAX;
use super::RecoveryError;
use super::RecoveryScope;
use super::RegistrationFailure;
use crate::operation::actor::OperationActor;
use carosign_core::ids::MutationId;
use carosign_engine::EffectSpec;
use carosign_engine::OperationBody;
use carosign_engine::OperationPhase;
use carosign_engine::OperationState;
use std::collections::BTreeMap;

/// Coordinator of closed areas of one running service.
#[derive(Default)]
pub struct RecoveryCoordinator {
    scopes: BTreeMap<RecoveryScope, Entry>,
}

impl RecoveryCoordinator {
    /// Creates an empty coordinator before loading a sustainable log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Encloses the area; the source list gives a check of the journal.
    pub fn block(
        &mut self,
        scope: RecoveryScope,
        mutation: MutationId,
    ) -> Result<(), RecoveryError> {
        if self
            .scopes
            .get(&scope)
            .is_some_and(|entry| entry.mutation == mutation)
        {
            return Ok(());
        }
        if self.scopes.contains_key(&scope)
            || self.scopes.values().any(|entry| entry.mutation == mutation)
        {
            return Err(RecoveryError::Conflict);
        }
        if self.scopes.len() >= RECOVERY_SCOPES_MAX {
            return Err(RecoveryError::Limit);
        }
        self.scopes.insert(
            scope,
            Entry {
                mutation,
                actor: None,
            },
        );
        Ok(())
    }

    /// Holds the restored operation in an already closed area.
    pub fn register(&mut self, actor: OperationActor) -> Result<(), RegistrationFailure> {
        let scope = match self.check_actor(&actor) {
            Ok(scope) => scope,
            Err(reason) => {
                return Err(RegistrationFailure {
                    reason,
                    actor: Box::new(actor),
                });
            }
        };
        // There are no external calls to the insert, the verified entry cannot change between
        // verification and transfer of possession.
        self.scopes
            .get_mut(&scope)
            .expect("область проверена")
            .actor = Some(actor);
        Ok(())
    }

    fn check_actor(&self, actor: &OperationActor) -> Result<RecoveryScope, RecoveryError> {
        let state = actor.state();
        let mutation = state.mutation().ok_or(RecoveryError::InvalidActor)?;
        let plan = mutation.plan.as_ref().ok_or(RecoveryError::InvalidActor)?;
        // The state fields are still open: reconfigure the plan with the query and context
        // before taking the actor into the enclosed area.
        let checked = match &state.body {
            OperationBody::Enrollment { request, .. } => OperationState::enrollment_with_plan(
                state.common.context.clone(),
                request.clone(),
                mutation.mutation,
                plan.clone(),
            ),
            OperationBody::Removal { request, .. } => OperationState::removal_with_plan(
                state.common.context.clone(),
                request.clone(),
                mutation.mutation,
                plan.clone(),
            ),
            OperationBody::Verification(_) => return Err(RecoveryError::InvalidActor),
        }
        .map_err(|_| RecoveryError::InvalidActor)?;
        // The waiting effects of the initial attempt cannot be transferred to the
        // reconciliation: re-training or arm does not restore the right to send.
        if state.common.phase != OperationPhase::Reconciling
            || checked.mutation().map(|value| value.planned_new) != Some(mutation.planned_new)
            || state.common.pending_effects.iter().any(|effect| {
                matches!(
                    effect.spec,
                    EffectSpec::PersistIntent
                        | EffectSpec::PersistDispatchArmed
                        | EffectSpec::SendMutatingCommand
                )
            })
        {
            return Err(RecoveryError::InvalidActor);
        }
        let scope = RecoveryScope {
            subject: state.common.context.subject,
            authenticator: state.common.context.authenticator.id,
        };
        let entry = self.scopes.get(&scope).ok_or(RecoveryError::NotBlocked)?;
        if entry.mutation != mutation.mutation || entry.actor.is_some() {
            return Err(RecoveryError::Conflict);
        }
        Ok(scope)
    }

    /// Is the area blocked for dependent operations?
    pub fn is_blocked(&self, scope: RecoveryScope) -> bool {
        self.scopes.contains_key(&scope)
    }

    /// Is there a restored actor in the enclosed area?
    pub fn has_actor(&self, scope: RecoveryScope) -> bool {
        self.scopes
            .get(&scope)
            .is_some_and(|entry| entry.actor.is_some())
    }
}

#[path = "recovery_coordinator.tests.rs"]
#[cfg(test)]
mod tests;
