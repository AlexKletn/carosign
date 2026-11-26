use super::{
    CleanupScope, Duration, Error, IdSource, Instant, LocalResourceOwner, Payload, ProviderFrame,
    ProviderSupervisor, VerificationTransport, common, provider, relay_scope, remaining,
};

fn scope_is_live(resources: &LocalResourceOwner, lease: &CleanupScope) -> bool {
    resources.scheduler().lock().is_ok_and(|scheduler| {
        scheduler.holds_live_scope(lease.lease_id(), lease.owner(), lease.resource_set())
    })
}

/// Preparing a channel for `VerificationFlow::run_channel`The output is left to the actor,
/// its origin is checked with the general scheduler; the owner of the operation stops and
/// cleans after the result, and if the caller makes an error, he must start the same cleaning
/// and not pass the unprepared channel further.
pub fn prepare_tunnel(
    supervisor: &mut ProviderSupervisor,
    resources: &LocalResourceOwner,
    lease: &CleanupScope,
    package_id: [u8; 16],
    profile_id: [u8; 16],
    timeout: Duration,
) -> Result<VerificationTransport, Error> {
    let deadline = Instant::now().checked_add(timeout).ok_or(Error::Timeout)?;
    prepare_tunnel_until(
        supervisor, resources, lease, package_id, profile_id, deadline, false,
    )
    .map(|(channel, _, _)| channel)
}

/// Prepare a channel of change and return the actual declared abilities.
pub(crate) fn prepare_mutation_tunnel(
    supervisor: &mut ProviderSupervisor,
    resources: &LocalResourceOwner,
    lease: &CleanupScope,
    package_id: [u8; 16],
    profile_id: [u8; 16],
    timeout: Duration,
    enrollment: bool,
) -> Result<(VerificationTransport, common::CapabilityDescriptor, bool), Error> {
    let deadline = Instant::now().checked_add(timeout).ok_or(Error::Timeout)?;
    prepare_tunnel_until(
        supervisor, resources, lease, package_id, profile_id, deadline, enrollment,
    )
}

pub(super) fn prepare_tunnel_until(
    supervisor: &mut ProviderSupervisor,
    resources: &LocalResourceOwner,
    lease: &CleanupScope,
    package_id: [u8; 16],
    profile_id: [u8; 16],
    deadline: Instant,
    enrollment: bool,
) -> Result<(VerificationTransport, common::CapabilityDescriptor, bool), Error> {
    let owner = lease.owner();
    if owner.worker != supervisor.worker_id()
        || package_id.iter().all(|b| *b == 0)
        || profile_id.iter().all(|b| *b == 0)
        || !scope_is_live(resources, lease)
    {
        return Err(Error::Context);
    }
    let resource_slots = supervisor
        .resource_slots_for_binding()
        .map_err(|_| Error::Unavailable)?;
    let mut tunnel = supervisor.take_tunnel().ok_or(Error::Unavailable)?;
    let hello = tunnel
        .receive_event(remaining(deadline)?)
        .map_err(|_| Error::Unavailable)?;
    let Some(Payload::WorkerHello(hello)) = hello.payload else {
        return Err(Error::Protocol);
    };
    let mut features = vec![provider::Feature::BasicOperations as i32];
    let feedback = provider::Feature::CaptureFeedbackV1 as i32;
    if hello.features.contains(&feedback) {
        features.push(feedback);
    }
    let transaction = provider::Feature::EnrollmentTransactionV1 as i32;
    let rollback_enabled = enrollment && hello.features.contains(&transaction);
    if rollback_enabled {
        features.push(transaction);
    }
    tunnel
        .command(
            ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_MAX,
                worker_instance_id: owner.worker.as_bytes().to_vec(),
                frame_seq: 0,
                command_id: vec![],
                operation_id: vec![],
                device_epoch: 0,
                uid: None,
                payload: Some(Payload::Initialize(provider::Initialize {
                    worker_instance_id: owner.worker.as_bytes().to_vec(),
                    selected_minor: carosign_protocol::limits::MINOR_MAX,
                    package_id: package_id.to_vec(),
                    profile_id: profile_id.to_vec(),
                    enabled_features: features,
                })),
            },
            remaining(deadline)?,
        )
        .map_err(|_| Error::Unavailable)?;
    let initialized = tunnel
        .receive_event(remaining(deadline)?)
        .map_err(|_| Error::Unavailable)?;
    if !matches!(initialized.payload, Some(Payload::Initialized(ref value)) if value.ready_for_binding)
        || !scope_is_live(resources, lease)
    {
        return Err(Error::Context);
    }
    let command = carosign_linux::ids::KernelIds
        .new_command_id()
        .map_err(|_| Error::Unavailable)?;
    let authenticator = common::AuthenticatorRef {
        authenticator_id: owner.authenticator.id.as_bytes().to_vec(),
        device_epoch: owner.authenticator.device_epoch.value(),
    };
    let durable = supervisor.requires_durable_binding();
    resources
        .with_serialized_binding(lease, durable, || -> Result<(), Error> {
            let mut send = || {
                let timeout = remaining(deadline)?;
                // Even a sending error does not prove that the broker did not receive
                // the request.
                supervisor.mark_resource_binding_started();
                tunnel
                    .bind(
                        relay_scope(lease),
                        resource_slots
                            .iter()
                            .map(
                                |(id, role)| carosign_protocol::carosign::broker_relay::v1::Slot {
                                    resource_id: id.to_vec(),
                                    role: *role,
                                },
                            )
                            .collect(),
                        ProviderFrame {
                            major: 1,
                            minor: carosign_protocol::limits::MINOR_MAX,
                            worker_instance_id: owner.worker.as_bytes().to_vec(),
                            frame_seq: 0,
                            command_id: command.as_bytes().to_vec(),
                            operation_id: owner.operation.as_bytes().to_vec(),
                            device_epoch: owner.authenticator.device_epoch.value(),
                            uid: None,
                            payload: Some(Payload::BindAuthenticator(
                                provider::BindAuthenticator {
                                    authenticator: Some(authenticator.clone()),
                                    profile_id: profile_id.to_vec(),
                                    lease_id: lease.lease_id().as_bytes().to_vec(),
                                    fd_slots: resource_slots
                                        .iter()
                                        .enumerate()
                                        .map(|(index, (id, role))| provider::FdSlot {
                                            index: index as u32,
                                            resource_id: id.to_vec(),
                                            role: *role,
                                        })
                                        .collect(),
                                },
                            )),
                        },
                        timeout,
                    )
                    .map_err(|_| Error::Unavailable)
            };
            send()
        })
        .map_err(|_| Error::Unavailable)??;
    let ready = tunnel
        .receive_event(remaining(deadline)?)
        .map_err(|_| Error::Unavailable)?;
    let Some(Payload::AuthenticatorReady(value)) = ready.payload else {
        return Err(Error::Context);
    };
    if value.authenticator.as_ref() != Some(&authenticator) || !scope_is_live(resources, lease) {
        return Err(Error::Context);
    }
    let mut capabilities = value.capabilities.ok_or(Error::Context)?;
    if capabilities.modality != supervisor.modality() as i32 {
        return Err(Error::Context);
    }
    carosign_protocol::validation::normalize_capture_metadata(&mut capabilities);
    Ok((
        VerificationTransport::Tunnel(Box::new(tunnel)),
        capabilities,
        rollback_enabled,
    ))
}
