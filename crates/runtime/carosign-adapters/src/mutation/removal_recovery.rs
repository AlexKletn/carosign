use carosign_core::ids::MutationId;

use super::{RemovalRecoveryError, storage_failure::map_error};

/// Restores logical deletion after arm in the reconciliation phase.
///
/// The maintenance context is given by the trusted restorer in the current boot; it is not
/// the client's restored permission; you cannot infer the hardware status from the log; until
/// independent verification, the area remains closed, and command repetition is prohibited.
pub fn restore_removal_for_reconciliation(
    store: &dyn carosign_persistence::store::MetadataStore,
    context: carosign_core::operation::context::OperationContext,
    mutation: MutationId,
) -> Result<carosign_engine::OperationState, RemovalRecoveryError> {
    let plan = carosign_persistence::recovery::read_reconciliation_plan_v3(
        store,
        mutation,
        context.operation_id,
        context.subject,
        context.authenticator.id,
        context.authenticator.device_epoch,
    )
    .map_err(|error| RemovalRecoveryError::Storage(map_error(error)))?;
    restore_removal_from_plan(context, mutation, plan)
}

/// Restores the removal from the current v4 scheme for independent reconciliation.
/// The trusted restorer confirms the context; reading the plan does not give authority to
/// repeat the shipment and does not confirm the state of the device.
pub fn restore_removal_for_reconciliation_v4(
    store: &dyn carosign_persistence::store::MetadataStore,
    context: carosign_core::operation::context::OperationContext,
    mutation: MutationId,
) -> Result<carosign_engine::OperationState, RemovalRecoveryError> {
    let plan = carosign_persistence::mutation_store::read_reconciliation_plan_v4(
        store,
        mutation,
        context.operation_id,
        context.subject,
        context.authenticator.id,
        context.authenticator.device_epoch,
    )
    .map_err(|error| RemovalRecoveryError::Storage(map_error(error)))?;
    restore_removal_from_plan(context, mutation, plan)
}

fn restore_removal_from_plan(
    context: carosign_core::operation::context::OperationContext,
    mutation: MutationId,
    plan: carosign_core::operation::plan::MutationPlan,
) -> Result<carosign_engine::OperationState, RemovalRecoveryError> {
    use carosign_core::operation::{mutation::RemoveRequest, plan::PlanTransition};
    let PlanTransition::Remove { old } = plan.transition() else {
        return Err(RemovalRecoveryError::WrongTransition);
    };
    let request = RemoveRequest {
        subject: context.subject,
        enrollment: old,
    };
    let mut state =
        carosign_engine::OperationState::removal_with_plan(context, request, mutation, plan)
            .map_err(|_| RemovalRecoveryError::InvalidState)?;
    state.common.phase = carosign_engine::OperationPhase::Reconciling;
    carosign_engine::replay::rebuild(state, []).map_err(|_| RemovalRecoveryError::InvalidState)
}
