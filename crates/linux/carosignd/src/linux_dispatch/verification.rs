use std::sync::Arc;
use std::time::Duration;

use carosign_core::authorization::GrantScope;
use carosign_core::device::AuthenticatorRef;
use carosign_core::ids::IdSource;
use carosign_evidence::recipient::{ChallengeRef, RecipientRef};
use carosign_linux::ids::KernelIds;
use carosign_runtime::ports::TimeSource;

use crate::verification_owner::{
    AdvanceOutcome, FlowProgress, VerificationFlow, VerificationFlowTarget,
};

use super::DispatchError;

/// Technical category only: neither identifiers nor customer data are logged, and a snapshot
/// of the connection is taken separately at the time of failure.
fn flow_failure_reason(error: &crate::verification_owner::FlowError) -> &'static str {
    use crate::operations::IdempotentAdmission;
    use crate::verification_owner::FlowError;
    match error {
        FlowError::Admission(IdempotentAdmission::Existing(_)) => "existing",
        FlowError::Admission(IdempotentAdmission::Conflict) => "conflict",
        FlowError::Admission(IdempotentAdmission::Limit) => "limit",
        FlowError::Admission(IdempotentAdmission::Unavailable) => "unavailable",
        FlowError::Admission(IdempotentAdmission::Created(_)) => "unexpected_created",
        FlowError::Watch(_) => "watch",
        FlowError::ContextMismatch => "context_mismatch",
        FlowError::Operation(_) => "operation",
        FlowError::InvalidStage => "invalid_stage",
        _ => "other",
    }
}

/// Conducts one already disassembled request through the system policy and the provider.
/// The list of devices and receiving other requests belong to the calling service.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn verify<S, T>(
    service: &crate::service::Service<'_, S, crate::linux_policy::LinuxPolicy, KernelIds, T>,
    request: &carosign_linux_ipc::mapping::client::DomainRequest,
    request_id: &[u8],
    connection: &crate::server::Connection,
    handle: crate::operations::ConnectionHandle,
    profile: &crate::provider_supervisor::ProviderProfile,
    registry: Arc<crate::operations::OperationRegistry>,
    resources: Arc<crate::resources::LocalResourceOwner>,
    clock: Arc<T>,
    control: Arc<crate::control_watch::ControlOwner>,
    challenges: &crate::scoped_challenge::ScopedChallenges,
    queue: &super::DeviceQueue,
) -> Result<(), DispatchError>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    let caller =
        crate::cycle::caller_from_session(&connection.session).ok_or(DispatchError::Preparation)?;
    let epoch = profile.epoch();
    let (idempotency, intent) = crate::service::client_verification_intent(request)
        .map_err(|_| DispatchError::Preparation)?;
    let (subject, grant) = crate::accept::authorize_client_intent(
        service.authority,
        service.authority,
        &caller,
        &intent,
    )
    .await
    .map_err(|_| DispatchError::Preparation)?;
    if intent.authenticator != profile.identity().authenticator {
        return Err(DispatchError::Preparation);
    }
    let _turn = queue
        .wait(connection, intent.authenticator, || {
            service.authority.revalidate_now(&caller, &grant)
        })
        .map_err(DispatchError::Queue)?;
    let cycle = crate::accept::prepare_verified_intent(
        service.store,
        &caller,
        &intent,
        epoch,
        subject,
        &grant,
    )
    .map_err(|_| DispatchError::Preparation)?;
    let prepared = crate::service::PreparedClientVerification {
        caller: caller.clone(),
        idempotency,
        intent,
        cycle,
        grant,
    };
    let GrantScope::Verify {
        subject,
        authenticator,
        purpose,
    } = *prepared.grant.scope()
    else {
        return Err(DispatchError::Preparation);
    };
    let mut supervisor = crate::provider_supervisor::ProviderSupervisor::start(
        profile,
        *clock.boot_id().as_bytes(),
        epoch,
        Duration::from_secs(5),
    )
    .map_err(|_| DispatchError::Provider)?;
    let command = service
        .ids
        .new_command_id()
        .map_err(|_| DispatchError::Preparation)?;
    let challenge = ChallengeRef::accepted_by_recipient(
        carosign_linux::sys::random_16().map_err(|_| DispatchError::Preparation)?,
    )
    .ok_or(DispatchError::Preparation)?;
    let recipient = RecipientRef::new(
        carosign_linux::sys::random_16().map_err(|_| DispatchError::Preparation)?,
    )
    .ok_or(DispatchError::Preparation)?;
    let target = VerificationFlowTarget {
        worker: supervisor.worker_id(),
        command,
        authenticator: AuthenticatorRef {
            id: authenticator,
            device_epoch: epoch,
        },
        provider_subject: subject,
        uid: service
            .authority
            .uid_of_subject(subject)
            .map_err(|_| DispatchError::Preparation)?,
        purpose,
        challenge,
        recipient,
    };
    let binding = crate::operations::TrustedCallerBinding::from_connection(connection);
    let watch = service
        .authority
        .operation_watch_for(&caller, &prepared.grant)
        .map_err(DispatchError::AdmissionPolicy)?;
    let mut flow = VerificationFlow::admit_observing(
        service,
        &prepared,
        Some(&binding),
        handle,
        crate::operations::VERIFICATION_KEYS_MAX,
        registry.clone(),
        clock.clone(),
        control.clone(),
        resources.scheduler().clone(),
        target,
        Duration::from_secs(60),
        Some(watch),
    )
    .map_err(|error| DispatchError::AdmissionFlow {
        reason: flow_failure_reason(&error),
        connection_attached: registry.connection_is_attached(handle),
    })?;
    let accepted = crate::answer::encode(
        &flow
            .accepted_reply()
            .map_err(|error| DispatchError::AdmissionReply {
                reason: flow_failure_reason(&error),
                connection_attached: registry.connection_is_attached(handle),
            })?,
        request_id,
    );
    crate::server::write_frame(connection, &accepted).map_err(|_| DispatchError::Delivery)?;
    let scope = match flow.prepare_channel(service, &prepared).await {
        Ok(scope) => scope,
        Err(error) => {
            eprintln!("carosignd: подготовка канала проверки: {error:?}");
            match flow.retained_scope() {
                Ok(Some(scope)) => {
                    let cleanup = crate::resources::SupervisorCleanup::new_for_scope(
                        supervisor, resources, scope,
                    );
                    if let Err(error) = flow.cleanup_after_failure(&cleanup).await {
                        eprintln!("carosignd: уборка после сбоя подготовки канала: {error:?}");
                    }
                }
                Ok(None) => {}
                Err(error) => eprintln!("carosignd: аренда после сбоя подготовки: {error:?}"),
            }
            return Err(DispatchError::Channel);
        }
    };
    let channel = crate::supervised_verification::prepare_tunnel(
        &mut supervisor,
        &resources,
        &scope,
        *profile.identity().package.as_bytes(),
        *profile.identity().profile.as_bytes(),
        Duration::from_secs(5),
    );
    let cleanup = Arc::new(crate::resources::SupervisorCleanup::new_for_scope(
        supervisor,
        resources,
        scope.clone(),
    ));
    let mut channel = match channel {
        Ok(channel) => Some(channel),
        Err(error) => {
            eprintln!("carosignd: подготовка транспорта проверки: {error:?}");
            if let Err(error) = flow.cleanup_after_failure(cleanup.as_ref()).await {
                eprintln!("carosignd: уборка после сбоя подготовки транспорта: {error:?}");
            }
            return Err(DispatchError::Channel);
        }
    };
    let result = async {
        let trust = crate::scoped_verifier::LinuxScopedTrust::register(
            &registry,
            service.authority,
            &cleanup,
            &prepared,
            target,
            flow.operation(),
            challenges,
            clock.as_ref(),
        )
        .map_err(|error| {
            eprintln!("carosignd: регистрация доверенного контекста проверки: {error:?}");
            DispatchError::Evidence
        })?;
        let evidence = crate::scoped_verifier::ScopedEvidencePort {
            sources: crate::scoped_verifier::Sources {
                trust: &trust,
                challenges,
                store: registry.assertion_store(),
                ids: service.ids,
                clock: clock.as_ref(),
            },
        };
        let _cancel_reader = crate::cancel_reader::CancelReader::start(
            connection,
            flow.operation(),
            &registry,
            flow.cancellation_sender(),
            control,
            cleanup.clone(),
            None,
        )
        .map_err(|_| DispatchError::Channel)?;
        for _ in 0..4096 {
            if flow
                .drain_control(cleanup.as_ref(), std::future::pending)
                .await
                .map_err(|_| DispatchError::Channel)?
            {
                return deliver_cancelled(
                    &mut flow,
                    service,
                    &prepared,
                    &registry,
                    connection,
                    handle,
                    cleanup.as_ref(),
                )
                .await;
            }
            let result = flow
                .run_channel(
                    service,
                    &prepared,
                    channel.take(),
                    connection,
                    cleanup.as_ref(),
                    // The observer completes the operation at its 60-second time; the transport
                    // must not interrupt the waiting sensor before it.
                    Duration::from_secs(60),
                    std::future::pending,
                )
                .await;
            let progress = match result {
                Ok(progress) => progress,
                Err(error) => {
                    eprintln!("carosignd: выполнение проверки: {error:?}");
                    let drained = flow
                        .drain_control(cleanup.as_ref(), std::future::pending)
                        .await;
                    if let Err(error) = &drained {
                        eprintln!("carosignd: обработка управления проверкой: {error:?}");
                    }
                    if matches!(drained, Ok(true)) {
                        return deliver_cancelled(
                            &mut flow,
                            service,
                            &prepared,
                            &registry,
                            connection,
                            handle,
                            cleanup.as_ref(),
                        )
                        .await;
                    }
                    return Err(DispatchError::Channel);
                }
            };
            match progress {
                FlowProgress::Sent => return Ok(()),
                FlowProgress::WouldBlock => std::thread::yield_now(),
                FlowProgress::Other(AdvanceOutcome::Other(
                    crate::provider_verification::PumpEvent::Outcome(
                        crate::operations::ObservationProgress::Held(
                            carosign_runtime::operation::actor::ObservationError::EvidenceRequired,
                        ),
                    ),
                )) => {
                    let requirements = carosign_evidence::requirements::EvidenceRequirements {
                        purpose,
                        hardware_required: false,
                        fresh_challenge: true,
                    };
                    let assessment = flow
                        .evaluate_scoped_assertion(&evidence, &requirements)
                        .await
                        .map_err(|_| DispatchError::Evidence)?;
                    flow.prepare_scoped_positive(
                        assessment,
                        service.authority,
                        cleanup.as_ref(),
                        std::future::pending,
                    )
                    .await
                    .map_err(|_| DispatchError::Evidence)?;
                    let mut delivery = flow
                        .start_positive_delivery()
                        .map_err(|_| DispatchError::Delivery)?;
                    if prepared.intent.report_matched_enrollment {
                        delivery.report_matched_enrollment();
                    }
                    for _ in 0..4096 {
                        match delivery
                            .drive(connection, service.authority, clock.as_ref())
                            .await
                            .map_err(|_| DispatchError::Delivery)?
                        {
                            crate::operations::OwnerSendProgress::Sent => return Ok(()),
                            crate::operations::OwnerSendProgress::WouldBlock
                            | crate::operations::OwnerSendProgress::Advanced => {
                                std::thread::yield_now()
                            }
                        }
                    }
                    return Err(DispatchError::Delivery);
                }
                FlowProgress::Other(_) => {}
                FlowProgress::DispatchFailure(error) => {
                    eprintln!("carosignd: отправка проверки: {error:?}");
                    return Err(DispatchError::Channel);
                }
            }
        }
        Err(DispatchError::Channel)
    }
    .await;
    if result.is_err()
        && let Err(error) = flow.cleanup_after_failure(cleanup.as_ref()).await
    {
        eprintln!("carosignd: уборка после сбоя проверки: {error:?}");
    }
    result
}

async fn deliver_cancelled<S, T>(
    flow: &mut VerificationFlow,
    service: &crate::service::Service<'_, S, crate::linux_policy::LinuxPolicy, KernelIds, T>,
    prepared: &crate::service::PreparedClientVerification,
    registry: &crate::operations::OperationRegistry,
    connection: &crate::server::Connection,
    handle: crate::operations::ConnectionHandle,
    cleanup: &crate::resources::SupervisorCleanup,
) -> Result<(), DispatchError>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource,
{
    if registry
        .allow_cancelled_delivery(flow.operation(), &prepared.caller, handle)
        .is_err()
    {
        // The owner could have shut down. There's nothing left to deliver, but before the
        // handler closes, you still have to complete the confirmed cleaning.
        for _ in 0..4096 {
            let progress = flow
                .run_channel(
                    service,
                    prepared,
                    None,
                    connection,
                    cleanup,
                    Duration::from_secs(1),
                    std::future::pending,
                )
                .await;
            match progress {
                Ok(FlowProgress::DispatchFailure(crate::effect_owner::DriveOutcome::StepLimit)) => {
                    continue;
                }
                _ => break,
            }
        }
        return Err(DispatchError::Delivery);
    }
    for _ in 0..4096 {
        match flow
            .run_channel(
                service,
                prepared,
                None,
                connection,
                cleanup,
                Duration::from_secs(1),
                std::future::pending,
            )
            .await
            .map_err(|error| {
                #[cfg(test)]
                eprintln!("cancel delivery: {error:?}");
                let _ = error;
                DispatchError::Delivery
            })? {
            FlowProgress::Sent => return Ok(()),
            FlowProgress::WouldBlock => std::thread::yield_now(),
            _ => return Err(DispatchError::Delivery),
        }
    }
    Err(DispatchError::Delivery)
}
