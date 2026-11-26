use super::{
    MetadataStore, MutationId, MutationPhase, OperationId, StorageError, read_mutation_snapshot_v3,
};

/// Reads the full plan of the armed mutation for independent verification.
///
/// The log picture, the lockdown and the queue are checked together, the area and the era
/// expected must be provided by a trusted restorer, the return plan does not prove physical
/// condition and does not allow re-sending.
pub fn read_reconciliation_plan_v3(
    store: &dyn MetadataStore,
    mutation: MutationId,
    operation: OperationId,
    subject: carosign_core::identity::SubjectRef,
    authenticator: carosign_core::ids::AuthenticatorId,
    epoch: carosign_core::device::DeviceEpoch,
) -> Result<carosign_core::operation::plan::MutationPlan, StorageError> {
    let snapshot = read_mutation_snapshot_v3(store, mutation, operation)?;
    let record = snapshot.mutation;
    if record.subject != subject
        || record.authenticator != authenticator
        || record.epoch != Some(epoch)
    {
        return Err(StorageError::Corrupt {
            what: "область или эпоха восстановления не совпадает",
        });
    }
    if !matches!(
        record.phase,
        MutationPhase::DispatchArmed
            | MutationPhase::ProviderObserved
            | MutationPhase::NeedsReconciliation
    ) {
        return Err(StorageError::GenerationConflict);
    }
    record
        .plan
        .ok_or(StorageError::Corrupt {
            what: "нет плана восстановления",
        })?
        .to_core(crate::mutation_plan::Scope {
            subject,
            authenticator,
        })
        .map_err(|_| StorageError::Corrupt {
            what: "неверный план восстановления",
        })
}
