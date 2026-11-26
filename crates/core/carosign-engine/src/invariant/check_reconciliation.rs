use crate::{decision::TransitionError, transition::OperationState};

/// The saved result of the reconciliation must correspond to the selected action.
pub(crate) fn check_reconciliation(state: &OperationState) -> Result<(), TransitionError> {
    if let Some(mutation) = state.mutation()
        && let Some(verdict) = mutation.reconciliation
        && (verdict.registry_action().is_none() || verdict.registry_action() != mutation.registry)
    {
        return Err(TransitionError::NotSpecified {
            what: "итог сверки не соответствует действию с реестром",
        });
    }
    Ok(())
}
