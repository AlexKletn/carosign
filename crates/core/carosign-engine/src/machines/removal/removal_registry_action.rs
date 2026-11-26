/// Reconciliation for removal: confirmed deletion - `ApplyDesiredState`(a) the original
/// condition has been confirmed. `RecordNoChange`.
///
/// ⚠️ Both go through `Committing` (B-01); there is only the meaning of the action.
pub fn removal_registry_action(desired_present: bool) -> crate::reconcile::RegistryAction {
    if desired_present {
        crate::reconcile::RegistryAction::ApplyDesiredState
    } else {
        crate::reconcile::RegistryAction::RecordNoChange
    }
}
