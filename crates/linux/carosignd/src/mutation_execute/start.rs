use std::{sync::Arc, time::Duration};

use carosign_core::ids::IdSource;
use carosign_engine::{DomainInput, EffectReceipt, EffectSpec, PlannedEffect};
use carosign_runtime::ports::{PolicyAuthority, ProviderExecutionPort, ResourceOwner, TimeSource};

use super::{
    ActiveMutation, MutationRequest, MutationStartError, PreparedMutation, actor_effect,
    challenged_enrollment::ChallengedEnrollment,
};

/// Prior to the return, the right to a result has not yet been confirmed; the caller is
/// obliged to accept the event of the same session and perform a steady fixation and
/// cleaning.
#[allow(clippy::too_many_arguments)]
pub(super) async fn start_inner<S: carosign_persistence::store::MetadataStore>(
    prepared: &PreparedMutation,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    profile: &crate::provider_supervisor::ProviderProfile,
    resources: &Arc<crate::resources::LocalResourceOwner>,
    registry: &Arc<crate::operations::OperationRegistry>,
    connection: crate::operations::ConnectionHandle,
    clock: &dyn TimeSource,
    enrollment_authority: Arc<
        dyn carosign_runtime::enrollment_authorization::EnrollmentAuthorizationAuthority,
    >,
    monitor: Option<(
        &crate::server::Connection,
        Arc<crate::control_watch::ControlOwner>,
        Arc<dyn TimeSource>,
    )>,
) -> Result<ActiveMutation, MutationStartError> {
    use crate::resources::ResourceSetSource;
    use carosign_core::authorization::AuthorizationBoundary;
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::identity::OwnerRef;
    use carosign_core::operation::context::{BootDeadline, OperationContext, VerificationPurpose};
    use carosign_core::operation::mutation::{EnrollRequest, RemoveRequest};
    use carosign_persistence::journal::{ArmMutation, PrepareMutation};
    use carosign_persistence::model::IdempotencyRecord;
    use carosign_persistence::mutation_plan::{Scope, Transition};
    use carosign_runtime::ports::{
        CommandPlan, EnrollmentAuthorization, MutationArmRequest, MutationChange,
    };

    policy
        .revalidate_caller(
            &prepared.caller,
            &prepared.grant,
            AuthorizationBoundary::BeforeCreation,
        )
        .await
        .map_err(|_| MutationStartError::Authority)?;
    let ids = carosign_linux::ids::KernelIds;
    let operation = ids
        .new_operation_id()
        .map_err(|_| MutationStartError::Context)?;
    let mutation = ids
        .new_mutation_id()
        .map_err(|_| MutationStartError::Context)?;
    let command = ids
        .new_command_id()
        .map_err(|_| MutationStartError::Context)?;
    let authenticator = AuthenticatorRef {
        id: prepared.authenticator,
        device_epoch: profile.epoch(),
    };
    let scope = Scope {
        subject: prepared.subject,
        authenticator: prepared.authenticator,
    };
    let core_plan = prepared
        .plan
        .to_core(scope)
        .map_err(|_| MutationStartError::Context)?;
    let records = crate::accept::read_enrollment_records(
        store,
        &crate::accept::scope_of(prepared.subject, prepared.authenticator),
    )
    .map_err(|_| MutationStartError::Storage)?;
    let old_binding = |old: carosign_core::enrollment::EnrollmentRef| -> Result<carosign_runtime::EnrollmentBinding, MutationStartError> {
        let record = records.iter().find(|record| record.enrollment == old).ok_or(MutationStartError::Context)?;
        if record.binding_kind != carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque {
            return Err(MutationStartError::Context);
        }
        Ok(carosign_runtime::EnrollmentBinding { enrollment: old, kind: carosign_runtime::BindingKind::ProviderOpaque,
            provider_ref: record.provider_ref.as_bytes().to_vec(),
            format_id: (!record.format_id.is_empty()).then(|| record.format_id.clone()), format_version: record.format_version })
    };
    let change = match core_plan.transition() {
        carosign_core::operation::plan::PlanTransition::Create { planned_new } => {
            MutationChange::Create { planned_new }
        }
        carosign_core::operation::plan::PlanTransition::Replace { old, planned_new } => {
            MutationChange::Replace {
                planned_new,
                old: old_binding(old)?,
            }
        }
        carosign_core::operation::plan::PlanTransition::Remove { old } => {
            MutationChange::Removal {
                enrollment: old,
                binding: old_binding(old)?,
            }
        }
    };
    let context = OperationContext {
        operation_id: operation,
        subject: prepared.subject,
        authenticator,
        owner: OwnerRef {
            authority: prepared.grant.issuer(),
            binding: prepared.grant.id(),
        },
        purpose: VerificationPurpose::SessionLogin,
        deadline: BootDeadline::new(prepared.grant.boot(), prepared.grant.deadline_ns()),
    };
    let state = match &prepared.request {
        MutationRequest::Enroll { label, replace } => {
            carosign_engine::OperationState::enrollment_with_plan(
                context,
                EnrollRequest {
                    subject: prepared.subject,
                    authenticator: prepared.authenticator,
                    target: label.clone(),
                    replace: *replace,
                },
                mutation,
                core_plan.clone(),
            )
        }
        MutationRequest::Remove { enrollment } => {
            carosign_engine::OperationState::removal_with_plan(
                context,
                RemoveRequest {
                    subject: prepared.subject,
                    enrollment: *enrollment,
                },
                mutation,
                core_plan.clone(),
            )
        }
    }
    .map_err(|_| MutationStartError::Context)?;
    registry
        .reserve_mutation(
            carosign_runtime::OperationActor::new(state),
            prepared.grant.clone(),
            &prepared.caller,
            connection,
        )
        .map_err(|_| MutationStartError::State)?;
    let persist = actor_effect(registry, operation, None, EffectSpec::PersistIntent)?;
    let input = PrepareMutation {
        mutation,
        operation,
        subject: prepared.subject,
        authenticator: prepared.authenticator,
        expected_generation: prepared.plan.old_target.map(|old| old.generation),
        enroll: prepared.plan.transition != Transition::Remove,
        plan: prepared.plan.clone(),
    };
    let idem = IdempotencyRecord {
        caller_digest: prepared.caller.opaque,
        key: prepared.idempotency,
        request_digest: prepared.request_digest,
        operation,
        mutation: Some(mutation),
    };
    let fresh = carosign_persistence::mutation_store::prepare_fresh_arm_v4(store, &input, &idem)
        .map_err(|_| MutationStartError::Storage)?;
    let acquire = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: persist,
            receipt: EffectReceipt::IntentDurable,
        }),
        EffectSpec::AcquireResources,
    )?;
    let mut supervisor = crate::provider_supervisor::ProviderSupervisor::start(
        profile,
        *clock.boot_id().as_bytes(),
        authenticator.device_epoch,
        Duration::from_secs(5),
    )
    .map_err(|_| MutationStartError::Provider)?;
    let worker = supervisor.worker_id();
    let set = profile
        .identity()
        .resource_set(authenticator)
        .map_err(|_| MutationStartError::Resource)?;
    let lease = resources
        .acquire_mutation(
            ResourceOwner {
                operation,
                worker,
                authenticator,
            },
            &set,
        )
        .await
        .map_err(|_| MutationStartError::Resource)?;
    let lease_id = lease.lease_id();
    let cleanup_scope = crate::resources::CleanupScope::of(&lease);
    {
        let mut guard = registry
            .checkout(operation)
            .map_err(|_| MutationStartError::State)?;
        let owned = guard.operation_mut();
        owned.lease = Some(lease);
        owned.lease_matches_request = true;
    }
    let revalidate = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: acquire,
            receipt: EffectReceipt::LeaseGranted,
        }),
        EffectSpec::RevalidateGrant,
    )?;
    policy
        .revalidate_caller(
            &prepared.caller,
            &prepared.grant,
            AuthorizationBoundary::BeforeDispatch,
        )
        .await
        .map_err(|_| MutationStartError::Authority)?;
    let arm_effect = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: revalidate,
            receipt: EffectReceipt::GrantValid,
        }),
        EffectSpec::PersistDispatchArmed,
    )?;
    let send_effect = {
        let mut guard = registry
            .checkout(operation)
            .map_err(|_| MutationStartError::State)?;
        let index = guard
            .operation_mut()
            .actor
            .state()
            .common
            .transition_index
            .next()
            .and_then(|x| x.next())
            .map_err(|_| MutationStartError::State)?;
        PlannedEffect::at(EffectSpec::SendMutatingCommand, index, 0)
    };
    let arm = ArmMutation {
        mutation,
        worker,
        epoch: authenticator.device_epoch,
        command,
        recovery_ref: None,
    };
    let mut request = MutationArmRequest {
        operation,
        uid: policy
            .uid_of_subject(prepared.subject)
            .map_err(|_| MutationStartError::Context)?,
        effect: send_effect,
        arm: arm.clone(),
        scope: crate::accept::scope_of(prepared.subject, prepared.authenticator),
        lease: lease_id,
        plan: CommandPlan::Mutate {
            mutation,
            command,
            authorization: EnrollmentAuthorization::None,
            change,
        },
        logical_plan: core_plan,
        label: match &prepared.request {
            MutationRequest::Enroll { label, .. } => Some(label.clone()),
            MutationRequest::Remove { .. } => None,
        },
    };
    registry
        .bind_mutation_dispatch(&request, clock, policy, resources.as_ref())
        .await
        .map_err(|_| MutationStartError::State)?;
    let effects = crate::effect_owner::EffectOwner::default();
    registry
        .handoff_mutation_binding(operation, &effects)
        .map_err(|_| MutationStartError::State)?;
    let (channel, capabilities, rollback_enabled) =
        crate::supervised_verification::prepare_mutation_tunnel(
            &mut supervisor,
            resources,
            &cleanup_scope,
            *profile.identity().package.as_bytes(),
            *profile.identity().profile.as_bytes(),
            Duration::from_secs(5),
            matches!(&prepared.request, MutationRequest::Enroll { .. }),
        )
        .map_err(|_| MutationStartError::Provider)?;
    use carosign_protocol::carosign::common::v1::{
        EnrollmentAuthorizationMode, ProviderOperation, TemplateCustody,
    };
    let required_operation = match &prepared.request {
        MutationRequest::Enroll { .. } => ProviderOperation::ProviderEnroll,
        MutationRequest::Remove { .. } => ProviderOperation::ProviderRemove,
    };
    if !capabilities
        .operations
        .contains(&(required_operation as i32))
        || capabilities.template_custody != TemplateCustody::CustodyProviderManaged as i32
        || matches!(&prepared.request, MutationRequest::Enroll { .. })
            && !matches!(
                EnrollmentAuthorizationMode::from_i32(capabilities.enrollment_authorization),
                Some(
                    EnrollmentAuthorizationMode::EnrollAuthNone
                        | EnrollmentAuthorizationMode::EnrollAuthChallenged
                )
            )
    {
        return Err(MutationStartError::Provider);
    }
    let crate::provider_verification::VerificationTransport::Tunnel(mut channel) = channel else {
        return Err(MutationStartError::Provider);
    };
    let enrollment_authority: Arc<
        dyn carosign_runtime::enrollment_authorization::EnrollmentAuthorizationAuthority,
    > = if let Some(recipient) = profile.enrollment_recipient().filter(|_| {
        matches!(&prepared.request, MutationRequest::Enroll { .. })
            && capabilities.enrollment_authorization
                == EnrollmentAuthorizationMode::EnrollAuthChallenged as i32
    }) {
        let clock = monitor
            .as_ref()
            .map(|(_, _, clock)| clock.clone())
            .ok_or(MutationStartError::Authority)?;
        let operation = supervisor
            .take_enrollment_endpoint()
            .ok_or(MutationStartError::Authority)?;
        let control = supervisor
            .take_enrollment_control_endpoint()
            .ok_or(MutationStartError::Authority)?;
        let connector = crate::enrollment_connector::OwnerConnector::new(
            authenticator,
            recipient,
            operation,
            control,
        );
        Arc::new(
            carosign_linux_ipc::enrollment_authority_adapter::IpcEnrollmentAuthority::new(
                Arc::new(connector),
                clock,
                Duration::from_secs(30),
                Duration::from_secs(2),
            )
            .map_err(|_| MutationStartError::Authority)?,
        )
    } else {
        enrollment_authority
    };
    let cleanup = Arc::new(crate::resources::SupervisorCleanup::new_for_scope(
        supervisor,
        resources.clone(),
        cleanup_scope.clone(),
    ));
    let control = match monitor {
        Some((connection, owner, watch_clock)) => Some(Arc::new(
            crate::mutation_control::MutationControl::start(
                connection,
                operation,
                registry.clone(),
                owner,
                policy,
                prepared,
                cleanup.clone(),
                watch_clock,
                Duration::from_secs(60),
                rollback_enabled,
            )
            .map_err(|_| MutationStartError::State)?,
        )),
        None => None,
    };
    if control
        .as_ref()
        .is_some_and(|control| control.interrupted())
    {
        return Err(MutationStartError::Authority);
    }
    let mut authorization_guard = if matches!(&prepared.request, MutationRequest::Enroll { .. })
        && capabilities.enrollment_authorization
            == EnrollmentAuthorizationMode::EnrollAuthChallenged as i32
    {
        Some(
            ChallengedEnrollment {
                policy,
                profile,
                resources,
                registry,
                clock,
                cleanup: cleanup.as_ref(),
                cleanup_scope: &cleanup_scope,
                control: control.as_deref(),
            }
            .prepare(
                &mut channel,
                prepared,
                &mut request,
                enrollment_authority.clone(),
            )
            .await?,
        )
    } else {
        None
    };
    registry
        .check_mutation_arm_context(&request, clock.now())
        .map_err(|_| MutationStartError::State)?;
    let ticket = fresh
        .arm(&arm, send_effect)
        .map_err(|_| MutationStartError::Storage)?;
    let actual_send = actor_effect(
        registry,
        operation,
        Some(DomainInput::Receipt {
            effect: arm_effect,
            receipt: EffectReceipt::DispatchArmedDurable,
        }),
        EffectSpec::SendMutatingCommand,
    )?;
    if actual_send != send_effect {
        return Err(MutationStartError::State);
    }
    policy
        .revalidate_caller(
            &prepared.caller,
            &prepared.grant,
            AuthorizationBoundary::BeforeDispatch,
        )
        .await
        .map_err(|_| MutationStartError::Authority)?;
    if control
        .as_ref()
        .is_some_and(|control| control.interrupted())
    {
        return Err(MutationStartError::Authority);
    }
    if let Some(guard) = authorization_guard.as_ref() {
        let authorized = guard
            .authorization
            .as_ref()
            .ok_or(MutationStartError::Authority)?;
        enrollment_authority
            .revalidate(authorized, &guard.scope, &prepared.grant)
            .await
            .map_err(|_| MutationStartError::Authority)?;
        authorized
            .check(&guard.scope, &prepared.grant, clock.now())
            .map_err(|_| MutationStartError::Authority)?;
    }
    if control
        .as_ref()
        .is_some_and(|control| control.interrupted())
    {
        return Err(MutationStartError::Authority);
    }
    if !resources
        .scheduler()
        .lock()
        .map_err(|_| MutationStartError::Resource)?
        .holds_live_scope(
            cleanup_scope.lease_id(),
            cleanup_scope.owner(),
            cleanup_scope.resource_set(),
        )
    {
        return Err(MutationStartError::Resource);
    }
    // Authorization verification can wait for confirmation: after it, we re-check the policy
    // and deadline immediately before sending.
    if let Some(guard) = authorization_guard.as_ref() {
        policy
            .revalidate_caller(
                &prepared.caller,
                &prepared.grant,
                AuthorizationBoundary::BeforeDispatch,
            )
            .await
            .map_err(|_| MutationStartError::Authority)?;
        if control
            .as_ref()
            .is_some_and(|control| control.interrupted())
        {
            return Err(MutationStartError::Authority);
        }
        guard
            .authorization
            .as_ref()
            .ok_or(MutationStartError::Authority)?
            .check(&guard.scope, &prepared.grant, clock.now())
            .map_err(|_| MutationStartError::Authority)?;
        if !resources
            .scheduler()
            .lock()
            .map_err(|_| MutationStartError::Resource)?
            .holds_live_scope(
                cleanup_scope.lease_id(),
                cleanup_scope.owner(),
                cleanup_scope.resource_set(),
            )
        {
            return Err(MutationStartError::Resource);
        }
    }
    let bridge = carosign_adapters::provider::ProviderBridge::new(
        carosign_linux_ipc::provider::tunnel::TunnelProviderChannel::new(
            *channel,
            Duration::from_secs(30),
        ),
        *worker.as_bytes(),
        authenticator.device_epoch.value(),
    );
    if let Some(guard) = authorization_guard.as_mut() {
        use carosign_runtime::enrollment_admission::{
            ConsumptionOutcome, SerializedEnrollmentAdmission,
        };
        let authorized = guard
            .authorization
            .as_ref()
            .ok_or(MutationStartError::Authority)?;
        let authority = guard.authority.clone();
        let scope = guard.scope;
        let admission = Arc::new(
            SerializedEnrollmentAdmission::new(scope, authorized, move || {
                authority.revoke_now(&scope).is_ok()
            })
            .map_err(|_| MutationStartError::Authority)?,
        );
        cleanup
            .install_enrollment_admission(&admission)
            .map_err(|_| MutationStartError::Authority)?;
        let outcome = admission.consume(
            &scope,
            authorized,
            &prepared.grant,
            clock,
            store,
            &request,
            ticket,
            || {
                !control
                    .as_ref()
                    .is_some_and(|control| control.interrupted())
                    && resources.scheduler().lock().is_ok_and(|scheduler| {
                        scheduler.holds_live_scope(
                            cleanup_scope.lease_id(),
                            cleanup_scope.owner(),
                            cleanup_scope.resource_set(),
                        )
                    })
            },
            |ticket| {
                let mut boundary = |send: &mut dyn FnMut() -> bool| {
                    let scheduler = resources.scheduler();
                    let Ok(scheduler) = scheduler.lock() else {
                        return false;
                    };
                    if !scheduler.holds_live_scope(
                        cleanup_scope.lease_id(),
                        cleanup_scope.owner(),
                        cleanup_scope.resource_set(),
                    ) || admission.revocation_requested()
                        || control
                            .as_ref()
                            .is_some_and(|control| control.interrupted())
                    {
                        return false;
                    }
                    send()
                };
                if enrollment_authority.execute_owner(
                    authorized,
                    &scope,
                    &prepared.grant,
                    &mut boundary,
                ) != ConsumptionOutcome::Completed
                {
                    return ConsumptionOutcome::Unknown;
                }
                if admission.revocation_requested() {
                    return ConsumptionOutcome::Unknown;
                }
                if control
                    .as_ref()
                    .is_some_and(|control| control.interrupted())
                {
                    return ConsumptionOutcome::Unknown;
                }
                let mut continuation_boundary = |send: &mut dyn FnMut() -> bool| {
                    let scheduler = resources.scheduler();
                    let Ok(scheduler) = scheduler.lock() else {
                        return false;
                    };
                    if !scheduler.holds_live_scope(
                        cleanup_scope.lease_id(),
                        cleanup_scope.owner(),
                        cleanup_scope.resource_set(),
                    ) || admission.revocation_requested()
                        || control
                            .as_ref()
                            .is_some_and(|control| control.interrupted())
                    {
                        return false;
                    }
                    send()
                };
                let result =
                    bridge.continue_mutation(ticket, request.clone(), &mut continuation_boundary);
                if control
                    .as_ref()
                    .is_some_and(|control| control.interrupted())
                {
                    return ConsumptionOutcome::Unknown;
                }
                match result {
                    carosign_runtime::ports::DispatchOutcome::Accepted => {
                        ConsumptionOutcome::Completed
                    }
                    _ => ConsumptionOutcome::Unknown,
                }
            },
        );
        guard.admission = Some(admission);
        if outcome != Ok(ConsumptionOutcome::Completed) {
            return Err(MutationStartError::Provider);
        }
    } else if bridge.dispatch_mutation(ticket, request).await
        != carosign_runtime::ports::DispatchOutcome::Accepted
    {
        return Err(MutationStartError::Provider);
    }
    Ok(ActiveMutation {
        operation,
        mutation,
        command,
        worker,
        epoch: authenticator.device_epoch,
        effect: send_effect,
        cleanup,
        control,
        authorization: authorization_guard,
        rollback_enabled,
        channel: bridge.into_channel(),
        format_id: capabilities.template_format_id,
        format_version: capabilities.template_format_version,
    })
}

/// Production launch establishes supervision up to sustainable arm and shipment.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start<S: carosign_persistence::store::MetadataStore>(
    prepared: &PreparedMutation,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    profile: &crate::provider_supervisor::ProviderProfile,
    resources: &Arc<crate::resources::LocalResourceOwner>,
    registry: &Arc<crate::operations::OperationRegistry>,
    connection_handle: crate::operations::ConnectionHandle,
    connection: &crate::server::Connection,
    control: Arc<crate::control_watch::ControlOwner>,
    clock: Arc<dyn TimeSource>,
) -> Result<ActiveMutation, MutationStartError> {
    start_inner(
        prepared,
        store,
        policy,
        profile,
        resources,
        registry,
        connection_handle,
        clock.as_ref(),
        Arc::new(carosign_runtime::enrollment_authorization::UnavailableEnrollmentAuthority),
        Some((connection, control, clock.clone())),
    )
    .await
}
