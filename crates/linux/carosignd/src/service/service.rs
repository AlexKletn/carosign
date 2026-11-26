use core::fmt;

use carosign_core::device::AuthenticatorRef;
use carosign_core::device::DeviceEpoch;
use carosign_core::identity::{OwnerRef, SubjectRef};
use carosign_core::ids::{AuthenticatorId, IdSource, OperationId};
use carosign_core::operation::context::{BootInstant, VerificationPurpose};
use carosign_runtime::effect_runner::{DispatchPlans, EffectRunner};
use carosign_runtime::ports::{CallerContext, CommandPlan, PolicyAuthority, TimeSource};
use carosign_runtime::{OperationActor, VerificationCycle};

use crate::accept::{AcceptError, prepare_authorized_verification, prepare_client_verification};
use crate::answer::{Reply, error_of};
use crate::run::{RunOutcome, run};
use crate::serve::{Request, actor_for};

use super::{
    ClientPreparationError, PreparedClientVerification, Served,
    caller_bound_authority::CallerBoundAuthority,
};

/// Leasing number from the source.
fn lease_of(
    ids: &dyn IdSource,
) -> Result<carosign_core::ids::LeaseId, carosign_core::ids::IdUnavailable> {
    ids.new_lease_id()
}

/// What the service needs for one request.
pub struct Service<'a, S, A, I, T> {
    /// Register.
    pub store: &'a S,
    pub authority: &'a A,
    /// Identifier.
    pub ids: &'a I,
    pub clock: &'a T,
    /// Resources (leasing is provided by an broker).
    pub resources: &'a dyn carosign_runtime::ports::ResourcePort,
    /// The composition of the selected profile resources.
    pub resource_sets: &'a dyn crate::resources::ResourceSetSource,
    /// Owner of actors of accepted operations.
    pub operations: &'a crate::operations::OperationRegistry,
    pub provider: &'a dyn carosign_runtime::ports::ProviderExecutionPort,
    /// The magazine.
    pub mutation: &'a dyn carosign_runtime::ports::MutationPort,
}

impl<S, A, I, T> fmt::Debug for Service<'_, S, A, I, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Service(..)")
    }
}

impl<S, A, I, T> Service<'_, S, A, I, T>
where
    S: carosign_persistence::store::MetadataStore,
    A: PolicyAuthority,
    I: IdSource,
    T: TimeSource,
{
    /// Transmits the full request to sources of identity and policy, without launching the
    /// provider or substituting for the purpose chosen by the policy.
    pub async fn prepare_client_verification(
        &self,
        caller: &CallerContext,
        request: &carosign_linux_ipc::mapping::client::DomainRequest,
        identity: &dyn carosign_runtime::ports::IdentityAuthority,
        epoch: DeviceEpoch,
    ) -> Result<PreparedClientVerification, ClientPreparationError> {
        let (idempotency, intent) = client_verification_intent(request)?;
        let (cycle, grant) = prepare_client_verification(
            self.store,
            identity,
            self.authority,
            caller,
            &intent,
            epoch,
        )
        .await
        .map_err(ClientPreparationError::Refused)?;
        let now = self.clock.now();
        if grant.boot() != now.boot() || grant.deadline_ns() <= now.instant_ns() {
            return Err(ClientPreparationError::Refused(AcceptError::Unauthorized(
                carosign_runtime::ports::AuthorityRefusal::Expired,
            )));
        }
        Ok(PreparedClientVerification {
            caller: caller.clone(),
            idempotency,
            intent,
            cycle,
            grant,
        })
    }

    /// Publishes the prepared check and the idempotency key of one registry operation, the
    /// source of the link and the right to reissue the existing transaction are checked by
    /// the trusted team before the call.
    /// This step does not ask for resources and does not run the provider.
    #[allow(dead_code)]
    pub(crate) fn admit_client_verification(
        &self,
        prepared: &PreparedClientVerification,
        epoch: DeviceEpoch,
        binding: Option<&crate::operations::TrustedCallerBinding>,
        connection: crate::operations::ConnectionHandle,
        capacity: usize,
    ) -> crate::operations::IdempotentAdmission {
        use crate::operations::IdempotentAdmission;
        use carosign_core::authorization::GrantScope;

        let GrantScope::Verify {
            subject,
            authenticator,
            purpose,
        } = *prepared.grant.scope()
        else {
            return IdempotentAdmission::Unavailable;
        };
        let now = self.clock.now();
        if binding.is_none()
            || prepared.cycle.caller != prepared.caller
            || prepared.cycle.scope != *prepared.grant.scope()
            || prepared.intent.authenticator != authenticator
            || prepared.grant.boot() != now.boot()
            || prepared.grant.deadline_ns() <= now.instant_ns()
        {
            return IdempotentAdmission::Unavailable;
        }
        let request = Request {
            subject,
            authenticator: AuthenticatorRef {
                id: authenticator,
                device_epoch: epoch,
            },
            purpose,
            owner: OwnerRef {
                authority: prepared.grant.issuer(),
                binding: prepared.grant.id(),
            },
            deadline_ns: 60_000_000_000,
        };
        let Ok(context) = request.context(self.ids, self.clock) else {
            return IdempotentAdmission::Unavailable;
        };
        let after_context = self.clock.now();
        if context.deadline.is_expired_at(after_context)
            || prepared.grant.boot() != after_context.boot()
            || prepared.grant.deadline_ns() <= after_context.instant_ns()
        {
            return IdempotentAdmission::Unavailable;
        }
        let mut actor = actor_for(&request, context);
        if actor
            .bind_verification_scope(&prepared.grant, prepared.cycle.permitted.clone())
            .is_err()
        {
            return IdempotentAdmission::Unavailable;
        }
        self.operations.reserve_idempotent_verification(
            binding,
            prepared.idempotency,
            &prepared.intent,
            actor,
            prepared.grant.clone(),
            &prepared.caller,
            capacity,
            connection,
        )
    }

    /// Receives resources and conducts the accepted verification until the provider waits.
    /// The source of resources and authority belongs to the composition of the service; this
    /// step does not establish their trusted origin or send a command.
    #[allow(dead_code)]
    pub(crate) async fn prepare_admitted_dispatch(
        &self,
        prepared: &PreparedClientVerification,
        operation: OperationId,
        worker: carosign_core::ids::WorkerInstanceId,
    ) -> Result<RunOutcome, crate::operations::ObservationFailure> {
        use crate::operations::ObservationFailure;
        use carosign_engine::{EffectReceipt, EffectSpec, OperationPhase};

        let mut guard = self.operations.checkout(operation)?;
        let owned = guard.operation_mut();
        let state = owned.actor.state();
        let authenticator = state.common.context.authenticator;
        if state.common.context.operation_id != operation
            || state.common.phase != OperationPhase::New
            || !owned.actor.has_work()
            || owned.actor.has_control_work()
            || owned.lease.is_some()
            || !owned.belongs_to(&prepared.caller)
            || prepared.cycle.caller != prepared.caller
            || prepared.cycle.scope != *owned.grant.scope()
            || owned.actor.verification_scope() != Some(&prepared.cycle.permitted)
            || !crate::run::grant_matches_operation(&owned.actor, &owned.grant)
        {
            return Err(ObservationFailure::ContextMismatch);
        }
        if state
            .common
            .context
            .deadline
            .is_expired_at(self.clock.now())
        {
            return Err(ObservationFailure::DeadlineExpired);
        }
        if guard.delivery_invalidated() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let requested_set = self
            .resource_sets
            .resource_set(authenticator)
            .map_err(|_| ObservationFailure::ContextMismatch)?;
        let owner = carosign_runtime::ports::ResourceOwner {
            operation,
            worker,
            authenticator,
        };
        let acquired = self.resources.acquire_for(owner, &requested_set).await;
        let owned = guard.operation_mut();
        let granted = match acquired {
            Ok(lease) => {
                let matches = lease.owner() == owner
                    && lease.epoch() == authenticator.device_epoch
                    && lease.resource_set() == &requested_set;
                owned.lease_matches_request = matches;
                owned.lease = Some(lease);
                if !matches {
                    return Err(ObservationFailure::ContextMismatch);
                }
                true
            }
            Err(_) => false,
        };
        if guard.delivery_invalidated() {
            return Err(ObservationFailure::AwaitingActor);
        }
        let owned = guard.operation_mut();
        if owned
            .actor
            .state()
            .common
            .context
            .deadline
            .is_expired_at(self.clock.now())
        {
            return Err(ObservationFailure::DeadlineExpired);
        }
        let plans = DispatchPlans::verify(prepared.cycle.plan());
        let mut runner = EffectRunner::new(self.provider, self.mutation, plans);
        if let Some(lease) = owned.lease.as_ref() {
            runner = runner.with_scoped_cancel(lease, self.ids);
        }
        let authority = CallerBoundAuthority {
            authority: self.authority,
            caller: &prepared.caller,
        };
        Ok(run(
            &mut owned.actor,
            &mut runner,
            &authority,
            &owned.grant,
            &mut |_effect, spec| match spec {
                EffectSpec::AcquireResources => Some(if granted {
                    EffectReceipt::LeaseGranted
                } else {
                    EffectReceipt::LeaseDenied
                }),
                _ => None,
            },
        )
        .await)
    }
    /// Starts the check and takes steps.
    ///
    /// ⚠️ Order: recruitment and authority of the *do* actor; without them, there is no
    /// operation.
    pub async fn verify(
        &self,
        caller: &CallerContext,
        subject: SubjectRef,
        authenticator: AuthenticatorId,
        epoch: DeviceEpoch,
    ) -> Served {
        let (cycle, grant) = match prepare_authorized_verification(
            self.store,
            self.authority,
            caller,
            subject,
            authenticator,
            epoch,
        )
        .await
        {
            Ok(cycle) => cycle,
            Err(error) => return Served::Refused(Reply::Refused(error_of(&error))),
        };

        let authenticator_ref = AuthenticatorRef {
            id: authenticator,
            device_epoch: epoch,
        };
        let requested_set = match self.resource_sets.resource_set(authenticator_ref) {
            Ok(set) => set,
            Err(_) => {
                return Served::Refused(Reply::Refused(error_of(
                    &AcceptError::IdentityUnavailable,
                )));
            }
        };
        let request = Request {
            subject,
            authenticator: authenticator_ref,
            purpose: VerificationPurpose::SessionLogin,
            owner: OwnerRef {
                authority: grant.issuer(),
                // ⚠️ The owner’s binding is the permit number, not the transaction number:
                // the transaction number is not a right.
                binding: grant.id(),
            },
            deadline_ns: 30_000_000_000,
        };
        let context = match request.context(self.ids, self.clock) {
            Ok(context) => context,
            Err(_) => {
                return Served::Refused(Reply::Refused(error_of(
                    &AcceptError::IdentityUnavailable,
                )));
            }
        };
        // Prior to the publication of the transaction: the source failure does not leave the
        // occupied operation.
        let requested_lease = match lease_of(self.ids) {
            Ok(id) => id,
            Err(_) => {
                return Served::Refused(Reply::Refused(error_of(
                    &AcceptError::IdentityUnavailable,
                )));
            }
        };
        if context.deadline.is_expired_at(self.clock.now()) {
            return Served::Refused(Reply::Refused(error_of(&AcceptError::Unauthorized(
                carosign_runtime::ports::AuthorityRefusal::Expired,
            ))));
        }
        let operation = context.operation_id;
        // ⚠️ Outside is given a term as a pair of “load number and moment”: the client
        // compares it with his idea of download.
        let deadline = BootInstant::new(context.deadline.boot(), context.deadline.instant_ns());

        let mut actor: OperationActor = actor_for(&request, context);
        if actor
            .bind_verification_scope(&grant, cycle.permitted.clone())
            .is_err()
        {
            return Served::Refused(Reply::Refused(error_of(&AcceptError::Unauthorized(
                carosign_runtime::ports::AuthorityRefusal::Denied,
            ))));
        }
        if crate::serve::admit(&mut actor).is_err() {
            return Served::Refused(Reply::Refused(error_of(&AcceptError::IdentityUnavailable)));
        }

        if !crate::run::grant_matches_operation(&actor, &grant) {
            return Served::Refused(Reply::Refused(error_of(&AcceptError::Unauthorized(
                carosign_runtime::ports::AuthorityRefusal::Denied,
            ))));
        }

        let mut operation_guard = match self.operations.reserve(actor, grant, caller) {
            Ok(guard) => guard,
            Err(error) => {
                let code = match error {
                    crate::operations::AdmissionFailure::Limit => {
                        carosign_protocol::carosign::common::v1::ErrorCode::ErrorLimitExceeded
                    }
                    crate::operations::AdmissionFailure::Duplicate => {
                        carosign_protocol::carosign::common::v1::ErrorCode::ErrorUnavailable
                    }
                };
                return Served::Refused(Reply::Refused(
                    carosign_protocol::carosign::common::v1::Error {
                        code: code as i32,
                        safe_message: "operation unavailable".into(),
                    },
                ));
            }
        };

        // ⚠️ Leasing asks 'do' steps: without him, the operation won't go any further
        // `WaitingLease`, and it'll be fair.
        let resource_answer = self
            .resources
            .acquire(&requested_set, requested_lease)
            .await;
        let (granted, matching_lease) = match resource_answer {
            Ok(lease) => {
                let matches = lease.lease_id() == requested_lease
                    && lease.owner().operation == operation
                    && lease.owner().authenticator == authenticator_ref
                    && lease.epoch() == epoch
                    && lease.resource_set() == &requested_set;
                operation_guard.operation_mut().lease_matches_request = matches;
                operation_guard.operation_mut().lease = Some(lease);
                (true, matches)
            }
            Err(_) => (false, true),
        };
        if operation_guard
            .operation_mut()
            .actor
            .state()
            .common
            .context
            .deadline
            .is_expired_at(self.clock.now())
        {
            // This is only a ban on shipment, not a confirmation of quarantine.
            return Served::Ran {
                operation,
                deadline,
                outcome: RunOutcome::Refused {
                    phase: operation_guard.operation_mut().actor.state().common.phase,
                    reason: "срок истёк при ожидании ресурсов".into(),
                },
            };
        }
        if !matching_lease {
            // It's not. LeaseDenied: the broker may have given out something. Peace and
            // release are not confirmed, sending to the provider is prohibited.
            return Served::Ran {
                operation,
                deadline,
                outcome: RunOutcome::Refused {
                    phase: operation_guard.operation_mut().actor.state().common.phase,
                    reason: "выдача ресурсов не соответствует операции".into(),
                },
            };
        }

        // ⚠️ The test plan is from the set, not from the void.
        let plans = DispatchPlans::verify(cycle.plan());
        // ⚠️ Observations of resources and authority are done by the service: only the
        // service knows the answer, and it does so when the engine has had an effect, not
        // beforehand: the engine is waiting for observation, not for a sign.
        let owned = operation_guard.operation_mut();
        let mut runner = EffectRunner::new(self.provider, self.mutation, plans);
        if let Some(lease) = owned.lease.as_ref() {
            runner = runner.with_scoped_cancel(lease, self.ids);
        }
        let outcome = run(
            &mut owned.actor,
            &mut runner,
            self.authority,
            &owned.grant,
            &mut |_effect, spec| match spec {
                carosign_engine::EffectSpec::AcquireResources => Some(if granted {
                    carosign_engine::EffectReceipt::LeaseGranted
                } else {
                    carosign_engine::EffectReceipt::LeaseDenied
                }),
                _ => None,
            },
        )
        .await;
        Served::Ran {
            operation,
            deadline,
            outcome,
        }
    }

    /// Check command plan for the cycle (for checks).
    pub fn plan_of(cycle: &VerificationCycle) -> CommandPlan {
        cycle.plan()
    }

    /// Prepares a negative result for the original owner.
    /// The image does not confirm the sending or receipt by the customer.
    pub async fn prepare_completed_verification(
        &self,
        caller: &CallerContext,
        operation: OperationId,
    ) -> Result<crate::operations::PreparedOwnerVerification, crate::operations::OwnerDeliveryFailure>
    {
        self.operations
            .prepare_owner_verification(operation, caller, self.authority, self.clock)
            .await
    }

    /// Transmits a negative result to the retaining owner; loss of the owner before sending
    /// back the image to the current launch registry.
    #[allow(dead_code)]
    pub(crate) async fn prepare_negative_delivery<'a>(
        &'a self,
        caller: &CallerContext,
        operation: OperationId,
    ) -> Result<
        crate::operations::OwnerVerificationDelivery<'a>,
        crate::operations::OwnerDeliveryFailure,
    > {
        self.operations
            .prepare_owner_delivery(operation, caller, self.authority, self.clock)
            .await
    }
}

pub(crate) fn client_verification_intent(
    request: &carosign_linux_ipc::mapping::client::DomainRequest,
) -> Result<
    (
        carosign_core::ids::IdempotencyKey,
        carosign_runtime::authority::verification::VerificationIntent,
    ),
    ClientPreparationError,
> {
    let carosign_linux_ipc::mapping::client::DomainRequest::BeginVerification {
        idempotency,
        authenticator,
        subject,
        purpose_hint,
        enrollment,
        report_matched_enrollment,
        authority_request_ref,
    } = request
    else {
        return Err(ClientPreparationError::InvalidRequest);
    };
    let mut intent = carosign_runtime::authority::verification::VerificationIntent::from_wire(
        subject,
        *purpose_hint as i32,
        authority_request_ref,
        *authenticator,
    )
    .map_err(|_| ClientPreparationError::InvalidRequest)?;
    intent.report_matched_enrollment = *report_matched_enrollment;
    if let Some(enrollment) = enrollment {
        use carosign_core::{
            enrollment::{EnrollmentGeneration, EnrollmentRef},
            ids::EnrollmentId,
        };
        if enrollment.authenticator_id != authenticator.as_bytes().as_slice() {
            return Err(ClientPreparationError::InvalidRequest);
        }
        intent.enrollment = Some(EnrollmentRef {
            id: EnrollmentId::try_from(enrollment.enrollment_id.as_slice())
                .map_err(|_| ClientPreparationError::InvalidRequest)?,
            authenticator: *authenticator,
            generation: EnrollmentGeneration::new(enrollment.generation)
                .map_err(|_| ClientPreparationError::InvalidRequest)?,
        });
    }
    Ok((*idempotency, intent))
}
