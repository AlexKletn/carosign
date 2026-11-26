use carosign_core::ids::OperationId;

use super::MutationStartError;

pub(super) async fn release_mutation_resources(
    operation: OperationId,
    cleanup: &crate::resources::SupervisorCleanup,
    resources: &crate::resources::LocalResourceOwner,
    registry: &crate::operations::OperationRegistry,
) -> Result<(), MutationStartError> {
    let scope = {
        let mut guard = registry
            .checkout(operation)
            .map_err(|_| MutationStartError::State)?;
        let lease = guard
            .operation_mut()
            .lease
            .as_ref()
            .ok_or(MutationStartError::Resource)?;
        crate::resources::CleanupScope::of(lease)
    };
    let scheduler = resources.scheduler();
    let held = scheduler
        .lock()
        .map_err(|_| MutationStartError::Resource)?
        .begin_quiescing(scope.lease_id(), scope.owner(), scope.resource_set())
        .ok_or(MutationStartError::Resource)?;
    let proof =
        carosign_runtime::scheduler::cleanup::confirm_quiesced_for(cleanup, &scope, &held).await;
    let released = match proof {
        Ok(proof) => {
            carosign_runtime::scheduler::cleanup::confirm_released(cleanup, &scope, proof).await
        }
        Err(error) => Err(error),
    };
    let mut scheduler = scheduler.lock().map_err(|_| MutationStartError::Resource)?;
    let released = match released {
        Ok(released) => released,
        Err(_) => {
            scheduler.quiescing_uncertain(held);
            return Err(MutationStartError::Resource);
        }
    };
    if let Err(held) = scheduler.finish_quiescing(held, released) {
        scheduler.quiescing_uncertain(held);
        return Err(MutationStartError::Resource);
    }
    drop(scheduler);
    registry
        .checkout(operation)
        .map_err(|_| MutationStartError::State)?
        .operation_mut()
        .lease
        .take()
        .ok_or(MutationStartError::Resource)?;
    Ok(())
}
