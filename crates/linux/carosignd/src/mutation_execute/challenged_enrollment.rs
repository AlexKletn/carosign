use std::{sync::Arc, time::Duration};

use carosign_core::{authorization::AuthorizationBoundary, ids::IdSource};
use carosign_linux_ipc::provider::tunnel::TunnelClient;
use carosign_runtime::{
    enrollment_authorization::{EnrollmentAuthorizationAuthority, EnrollmentAuthorizationScope},
    ports::{
        CommandPlan, EnrollmentAuthorization, MutationArmRequest, PolicyAuthority, TimeSource,
    },
};

use super::{
    EnrollmentAuthorizationGuard, MutationStartError, PreparedMutation, enrollment_challenge_frame,
};
use crate::{
    linux_policy::LinuxPolicy,
    mutation_control::MutationControl,
    operations::OperationRegistry,
    provider_supervisor::ProviderProfile,
    resources::{CleanupScope, LocalResourceOwner, SupervisorCleanup},
};

pub(super) struct ChallengedEnrollment<'a> {
    pub(super) policy: &'a LinuxPolicy,
    pub(super) profile: &'a ProviderProfile,
    pub(super) resources: &'a LocalResourceOwner,
    pub(super) registry: &'a OperationRegistry,
    pub(super) clock: &'a dyn TimeSource,
    pub(super) cleanup: &'a SupervisorCleanup,
    pub(super) cleanup_scope: &'a CleanupScope,
    pub(super) control: Option<&'a MutationControl>,
}

impl ChallengedEnrollment<'_> {
    pub(super) async fn prepare(
        self,
        channel: &mut TunnelClient,
        prepared: &PreparedMutation,
        request: &mut MutationArmRequest,
        enrollment_authority: Arc<dyn EnrollmentAuthorizationAuthority>,
    ) -> Result<EnrollmentAuthorizationGuard, MutationStartError> {
        let Self {
            policy,
            profile,
            resources,
            registry,
            clock,
            cleanup,
            cleanup_scope,
            control,
        } = self;
        let operation = request.operation;
        let mutation = request.arm.mutation;
        let worker = request.arm.worker;
        let command = request.arm.command;
        let authenticator = cleanup_scope.owner().authenticator;
        let lease_id = request.lease;
        let ids = carosign_linux::ids::KernelIds;
        let challenge_command = ids
            .new_command_id()
            .map_err(|_| MutationStartError::Context)?;
        let challenge_scope = EnrollmentAuthorizationScope {
            operation,
            mutation,
            challenge_command,
            command,
            subject: prepared.subject,
            uid: request.uid,
            authenticator,
            worker,
            lease: lease_id,
            boot: clock.boot_id(),
            recipient: enrollment_authority
                .recipient_for(authenticator)
                .map_err(|_| MutationStartError::Authority)?,
        };
        if !profile.challenged_owner_configured() {
            return Err(MutationStartError::Authority);
        }
        let mut guard = EnrollmentAuthorizationGuard {
            authority: enrollment_authority.clone(),
            scope: challenge_scope,
            authorization: None,
            admission: None,
            revoked: false,
        };
        let preparation_authority = enrollment_authority.clone();
        cleanup
            .install_preparation_revoker(Arc::new(move || {
                preparation_authority.revoke_now(&challenge_scope).is_ok()
            }))
            .map_err(|_| MutationStartError::Authority)?;
        enrollment_authority
            .assess_preparation(&challenge_scope, &prepared.grant)
            .await
            .map_err(|_| MutationStartError::Authority)?;
        policy
            .revalidate_caller(
                &prepared.caller,
                &prepared.grant,
                AuthorizationBoundary::BeforeDispatch,
            )
            .await
            .map_err(|_| MutationStartError::Authority)?;
        if control.as_ref().is_some_and(|c| c.interrupted()) {
            return Err(MutationStartError::Authority);
        }
        registry
            .check_mutation_arm_context(request, clock.now())
            .map_err(|_| MutationStartError::State)?;
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
        let deadline = std::time::Instant::now()
            .checked_add(Duration::from_secs(30))
            .ok_or(MutationStartError::Provider)?;
        channel
            .command(
                enrollment_challenge_frame(&challenge_scope),
                deadline
                    .checked_duration_since(std::time::Instant::now())
                    .ok_or(MutationStartError::Provider)?,
            )
            .map_err(|_| MutationStartError::Provider)?;
        let first = channel
            .receive_event(
                deadline
                    .checked_duration_since(std::time::Instant::now())
                    .ok_or(MutationStartError::Provider)?,
            )
            .map_err(|_| MutationStartError::Provider)?;
        if !matches!(&first.payload, Some(carosign_protocol::carosign::provider::v1::provider_frame::Payload::CommandAck(ack)) if ack.accepted && ack.rejection.is_none())
            || first.worker_instance_id != worker.as_bytes()
            || first.operation_id != operation.as_bytes()
            || first.command_id != challenge_command.as_bytes()
            || first.device_epoch != request.arm.epoch.value()
        {
            return Err(MutationStartError::Context);
        }
        let observation = {
            let mut boundary = |send: &mut dyn FnMut() -> bool| {
                let scheduler = resources.scheduler();
                let Ok(scheduler) = scheduler.lock() else {
                    return false;
                };
                if !scheduler.holds_live_scope(
                    cleanup_scope.lease_id(),
                    cleanup_scope.owner(),
                    cleanup_scope.resource_set(),
                ) || control
                    .as_ref()
                    .is_some_and(|control| control.interrupted())
                {
                    return false;
                }
                send()
            };
            enrollment_authority
                .prepare_owner_challenge(&challenge_scope, &prepared.grant, &mut boundary)
                .map_err(|_| MutationStartError::Authority)?
        };
        if control
            .as_ref()
            .is_some_and(|control| control.interrupted())
        {
            return Err(MutationStartError::Authority);
        }
        let plan_digest = carosign_persistence::mutation_plan::canonical_digest(
            carosign_persistence::mutation_plan::Scope {
                subject: prepared.subject,
                authenticator: prepared.authenticator,
            },
            &prepared.plan,
        )
        .map_err(|_| MutationStartError::Storage)?;
        enrollment_authority
            .bind_plan(
                &challenge_scope,
                &prepared.grant,
                &carosign_runtime::enrollment_authorization::EnrollmentPlanBinding::from_plan(
                    &request.logical_plan,
                    plan_digest,
                    &prepared.grant,
                )
                .map_err(|_| MutationStartError::Authority)?,
            )
            .map_err(|_| MutationStartError::Authority)?;
        let validated = enrollment_authority
            .validate_session(&challenge_scope, &observation, &prepared.grant)
            .await
            .map_err(|_| MutationStartError::Authority)?;
        validated
            .check(&challenge_scope, &prepared.grant, clock.now())
            .map_err(|_| MutationStartError::Authority)?;
        validated
            .check_observation(&observation)
            .map_err(|_| MutationStartError::Authority)?;
        let authorized = enrollment_authority
            .authorize(&validated, &prepared.grant)
            .await
            .map_err(|_| MutationStartError::Authority)?;
        authorized
            .check(&challenge_scope, &prepared.grant, clock.now())
            .map_err(|_| MutationStartError::Authority)?;
        authorized
            .challenge()
            .check_observation(&observation)
            .map_err(|_| MutationStartError::Authority)?;
        enrollment_authority
            .revalidate(&authorized, &challenge_scope, &prepared.grant)
            .await
            .map_err(|_| MutationStartError::Authority)?;
        authorized
            .check(&challenge_scope, &prepared.grant, clock.now())
            .map_err(|_| MutationStartError::Authority)?;
        if control.as_ref().is_some_and(|c| c.interrupted()) {
            return Err(MutationStartError::Authority);
        }
        let CommandPlan::Mutate { authorization, .. } = &mut request.plan else {
            return Err(MutationStartError::Context);
        };
        *authorization = EnrollmentAuthorization::Reference {
            reference: *authorized.reference(),
            challenge_digest: *authorized.digest(),
        };
        guard.authorization = Some(authorized);
        let authorized = guard
            .authorization
            .as_ref()
            .ok_or(MutationStartError::Authority)?;
        let binding = enrollment_authority
            .take_owner_binding(authorized, &challenge_scope, &prepared.grant)
            .map_err(|_| MutationStartError::Authority)?;
        enrollment_authority
            .bind_owner(authorized, &challenge_scope, &prepared.grant, binding)
            .map_err(|_| MutationStartError::Authority)?;
        Ok(guard)
    }
}
