use carosign_core::ids::OperationId;
use carosign_engine::{DomainInput, EffectSpec, PlannedEffect};
use carosign_runtime::OperationMsg;

use super::MutationStartError;

pub(super) fn actor_effect(
    registry: &crate::operations::OperationRegistry,
    operation: OperationId,
    message: Option<DomainInput>,
    expected: EffectSpec,
) -> Result<PlannedEffect, MutationStartError> {
    let mut guard = registry
        .checkout(operation)
        .map_err(|_| MutationStartError::State)?;
    let actor = &mut guard.operation_mut().actor;
    if let Some(message) = message {
        actor
            .post(OperationMsg::Domain(message))
            .map_err(|_| MutationStartError::State)?;
    }
    match actor.run_once() {
        carosign_runtime::operation::actor::Step::Advanced { effects, .. } => {
            let mut matches = effects.into_iter().filter(|effect| effect.spec == expected);
            let effect = matches.next().ok_or(MutationStartError::State)?;
            if matches.next().is_some() {
                return Err(MutationStartError::State);
            }
            Ok(effect)
        }
        _ => Err(MutationStartError::State),
    }
}

pub(super) fn actor_advance(
    registry: &crate::operations::OperationRegistry,
    operation: OperationId,
    input: DomainInput,
) -> Result<(), MutationStartError> {
    let mut guard = registry
        .checkout(operation)
        .map_err(|_| MutationStartError::State)?;
    let actor = &mut guard.operation_mut().actor;
    actor
        .post(OperationMsg::Domain(input))
        .map_err(|_| MutationStartError::State)?;
    match actor.run_once() {
        carosign_runtime::operation::actor::Step::Advanced { .. } => Ok(()),
        _ => Err(MutationStartError::State),
    }
}

pub(super) fn publish_mutation_completion(
    registry: &crate::operations::OperationRegistry,
    operation: OperationId,
) {
    registry.publish_completed_mutation(operation);
}
