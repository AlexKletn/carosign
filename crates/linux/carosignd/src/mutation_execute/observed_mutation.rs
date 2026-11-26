use carosign_core::ids::OperationId;
use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, PlannedEffect};
use carosign_runtime::ports::PolicyAuthority;

use super::{
    ActiveMutation, MutationRequest, MutationStartError, PreparedMutation, actor_effect,
    release_mutation_resources,
};

/// The result is session-related and persistently recorded; it is not yet fixed in the
/// registry.
pub(crate) struct ObservedMutation {
    pub(crate) active: ActiveMutation,
    pub(crate) query_effect: PlannedEffect,
    pub(crate) digest: [u8; 32],
    pub(crate) provider_ref: Vec<u8>,
}

/// It fixes the result, confirms the stop and clean up, and then fixes the terminal, and any
/// unknown outcome leaves a record for reconciliation.
async fn finish_inner<S: carosign_persistence::store::MetadataStore>(
    mut observed: ObservedMutation,
    prepared: &PreparedMutation,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    resources: &crate::resources::LocalResourceOwner,
    registry: &crate::operations::OperationRegistry,
    interrupted: &dyn Fn() -> bool,
) -> Result<OperationId, MutationStartError> {
    use carosign_core::authorization::AuthorizationBoundary;
    use carosign_persistence::enrollment_codec::{EnrollmentRecord, EnrollmentStatus};
    use carosign_persistence::mutation_plan::{Scope, StoredBindingKind};
    let operation = observed.active.operation;
    let mutation = observed.active.mutation;
    if interrupted() {
        return Err(MutationStartError::Authority);
    }
    policy
        .revalidate_caller(
            &prepared.caller,
            &prepared.grant,
            AuthorizationBoundary::BeforeDispatch,
        )
        .await
        .map_err(|_| MutationStartError::Authority)?;
    {
        let guard = registry
            .checkout(operation)
            .map_err(|_| MutationStartError::State)?;
        if guard.delivery_invalidated() {
            return Err(MutationStartError::Authority);
        }
    }
    let commit = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: observed.query_effect,
            receipt: EffectReceipt::TrustedProviderCompleted,
        }),
        EffectSpec::CommitDesiredRegistryAndJournal,
    )?;
    if interrupted() {
        return Err(MutationStartError::Authority);
    }
    let plan = prepared
        .plan
        .to_core(Scope {
            subject: prepared.subject,
            authenticator: prepared.authenticator,
        })
        .map_err(|_| MutationStartError::Context)?;
    let record = match (&prepared.request, plan.transition()) {
        (
            MutationRequest::Enroll { label, .. },
            carosign_core::operation::plan::PlanTransition::Create { planned_new }
            | carosign_core::operation::plan::PlanTransition::Replace { planned_new, .. },
        ) => Some(EnrollmentRecord {
            subject: prepared.subject,
            enrollment: planned_new,
            display_label: None,
            label: label.as_str().to_owned(),
            status: EnrollmentStatus::Active,
            provider_ref: carosign_core::enrollment::OpaqueProviderRef::new(
                observed.provider_ref.clone(),
            )
            .map_err(|_| MutationStartError::Context)?,
            format_id: observed.active.format_id.clone(),
            format_version: observed.active.format_version,
            binding_kind: StoredBindingKind::ProviderOpaque,
        }),
        (
            MutationRequest::Remove { .. },
            carosign_core::operation::plan::PlanTransition::Remove { .. },
        ) => None,
        _ => return Err(MutationStartError::Context),
    };
    carosign_persistence::mutation_store::commit_registry_v4(
        store,
        &carosign_persistence::mutation_records::RegistryUpdate {
            mutation,
            operation,
            command: observed.active.command,
            worker: observed.active.worker,
            epoch: observed.active.epoch,
            observation_digest: observed.digest,
            new_record: record.as_ref(),
        },
    )
    .map_err(|_| MutationStartError::Storage)?;
    if interrupted() {
        return Err(MutationStartError::Authority);
    }
    if let Some(authorization) = observed.active.authorization.as_mut() {
        authorization.revoke()?;
    }
    let release = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: commit,
            receipt: EffectReceipt::MetadataCommitted,
        }),
        EffectSpec::ReleaseResources,
    )?;
    release_mutation_resources(
        operation,
        observed.active.cleanup.as_ref(),
        resources,
        registry,
    )
    .await?;
    if interrupted() {
        return Err(MutationStartError::Authority);
    }
    let _record_terminal = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: release,
            receipt: EffectReceipt::CleanupConfirmed,
        }),
        EffectSpec::RecordTerminal,
    )?;
    carosign_persistence::mutation_store::complete_v4(store, mutation, operation)
        .map_err(|_| MutationStartError::Storage)?;
    super::actor::publish_mutation_completion(registry, operation);
    if interrupted() {
        return Err(MutationStartError::Authority);
    }
    Ok(operation)
}

/// The production path requires continuous supervision until the final result.
pub(crate) async fn finish_guarded<S: carosign_persistence::store::MetadataStore>(
    observed: ObservedMutation,
    prepared: &PreparedMutation,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    resources: &crate::resources::LocalResourceOwner,
    registry: &crate::operations::OperationRegistry,
    control: &crate::mutation_control::MutationControl,
) -> Result<OperationId, MutationStartError> {
    finish_inner(
        observed,
        prepared,
        store,
        policy,
        resources,
        registry,
        &|| control.interrupted(),
    )
    .await
}

#[cfg(test)]
pub(super) async fn finish<S: carosign_persistence::store::MetadataStore>(
    observed: ObservedMutation,
    prepared: &PreparedMutation,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    resources: &crate::resources::LocalResourceOwner,
    registry: &crate::operations::OperationRegistry,
) -> Result<OperationId, MutationStartError> {
    finish_inner(
        observed,
        prepared,
        store,
        policy,
        resources,
        registry,
        &|| false,
    )
    .await
}
