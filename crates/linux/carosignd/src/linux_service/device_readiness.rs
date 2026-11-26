use super::{Arc, CapabilityState, TimeSource};

pub(super) fn device_readiness(
    declared: Option<carosign_protocol::carosign::common::v1::CapabilityDescriptor>,
    probe: impl FnOnce()
        -> std::io::Result<carosign_protocol::carosign::common::v1::CapabilityDescriptor>,
) -> CapabilityState {
    let mut confirmed = if crate::linux_dispatch::available_capabilities(declared.as_ref(), true) {
        probe().ok()
    } else {
        None
    };
    if let (Some(capabilities), Some(declaration)) = (confirmed.as_mut(), declared.as_ref()) {
        if capabilities.assurance.is_none() {
            capabilities.assurance = declaration.assurance.clone();
        }
        capabilities.enrollment_stages = capabilities
            .enrollment_stages
            .or(declaration.enrollment_stages);
    }
    CapabilityState {
        verified_ready: confirmed.is_some(),
        descriptor: confirmed.or(declared),
    }
}

/// Does not confirm the presence/readiness of the equipment and does not launch the owner.
pub(crate) fn probe_declaration<T: TimeSource>(
    profile: &crate::provider_supervisor::ProviderProfile,
    clock: &T,
) -> std::io::Result<carosign_protocol::carosign::common::v1::CapabilityDescriptor> {
    use carosign_core::ids::IdSource;
    use carosign_protocol::carosign::{
        common::v1 as common,
        provider::v1::{self as provider, ProviderFrame, provider_frame::Payload},
    };
    let timeout = std::time::Duration::from_secs(5);
    let deadline = std::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| std::io::Error::other("срок декларации"))?;
    let remaining = || {
        deadline
            .checked_duration_since(std::time::Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| std::io::Error::other("срок декларации истёк"))
    };
    let epoch = profile.epoch();
    let mut supervisor = crate::provider_supervisor::ProviderSupervisor::start_declaration(
        profile,
        *clock.boot_id().as_bytes(),
        epoch,
        remaining()?,
    )?;
    let mut tunnel = supervisor
        .take_tunnel()
        .ok_or_else(|| std::io::Error::other("нет канала декларации"))?;
    let hello = tunnel
        .receive_event(remaining()?)
        .map_err(|_| std::io::Error::other("нет приветствия провайдера"))?;
    let Some(Payload::WorkerHello(hello)) = hello.payload else {
        return Err(std::io::Error::other("неверное приветствие"));
    };
    if hello.major != 1
        || hello.min_minor > carosign_protocol::limits::MINOR_MAX
        || hello.max_minor < carosign_protocol::limits::MINOR_BASELINE
        || !hello
            .features
            .contains(&(provider::Feature::BasicOperations as i32))
        || !hello
            .features
            .contains(&(provider::Feature::PrebindDescriptionV1 as i32))
    {
        return Err(std::io::Error::other(
            "провайдер не поддерживает декларацию",
        ));
    }
    let worker = supervisor.worker_id().as_bytes().to_vec();
    tunnel
        .command(
            ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_MAX,
                worker_instance_id: worker.clone(),
                payload: Some(Payload::Initialize(provider::Initialize {
                    worker_instance_id: worker.clone(),
                    selected_minor: carosign_protocol::limits::MINOR_MAX,
                    package_id: profile.identity().package.as_bytes().to_vec(),
                    profile_id: profile.identity().profile.as_bytes().to_vec(),
                    enabled_features: vec![
                        provider::Feature::BasicOperations as i32,
                        provider::Feature::PrebindDescriptionV1 as i32,
                    ],
                })),
                ..Default::default()
            },
            remaining()?,
        )
        .map_err(|_| std::io::Error::other("инициализация декларации"))?;
    let initialized = tunnel
        .receive_event(remaining()?)
        .map_err(|_| std::io::Error::other("нет Initialized"))?;
    if !matches!(initialized.payload, Some(Payload::Initialized(ref value)) if value.ready_for_binding)
    {
        return Err(std::io::Error::other("неверный Initialized"));
    }
    let authenticator = common::AuthenticatorRef {
        authenticator_id: profile.identity().authenticator.as_bytes().to_vec(),
        device_epoch: epoch.value(),
    };
    let command = carosign_linux::ids::KernelIds
        .new_command_id()
        .map_err(|_| std::io::Error::other("нет ID декларации"))?;
    let description = tunnel
        .prebind_describe(
            ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_MAX,
                worker_instance_id: worker,
                command_id: command.as_bytes().to_vec(),
                device_epoch: epoch.value(),
                payload: Some(Payload::PrebindDescribe(provider::PrebindDescribe {
                    authenticator: Some(authenticator),
                })),
                ..Default::default()
            },
            remaining()?,
        )
        .map_err(|_| std::io::Error::other("декларация провайдера недоступна"))?;
    let Some(Payload::PrebindDescription(value)) = description.payload else {
        return Err(std::io::Error::other("неверная декларация"));
    };
    let mut capabilities = value
        .capabilities
        .ok_or_else(|| std::io::Error::other("нет возможностей"))?;
    if capabilities.modality != profile.identity().modality as i32 {
        return Err(std::io::Error::other("вид устройства не совпал"));
    }
    drop(tunnel);
    drop(supervisor);
    carosign_protocol::validation::normalize_capture_metadata(&mut capabilities);
    Ok(capabilities)
}

/// Tests abilities through a regular Bind and then frees up the entire resource.
pub(crate) fn probe_capabilities<T: TimeSource>(
    profile: &crate::provider_supervisor::ProviderProfile,
    resources: &crate::resources::LocalResourceOwner,
    clock: &T,
) -> std::io::Result<carosign_protocol::carosign::common::v1::CapabilityDescriptor> {
    use crate::resources::ResourceSetSource;
    use carosign_core::device::AuthenticatorRef;
    use carosign_core::ids::IdSource;
    use carosign_runtime::ports::{ResourceOwner, ResourcePort};
    if resources.has_recovered_fence(&profile.identity().resources)? {
        return Err(std::io::Error::other(
            "ресурс остаётся ограждённым после восстановления",
        ));
    }
    let mut supervisor = crate::provider_supervisor::ProviderSupervisor::start(
        profile,
        *clock.boot_id().as_bytes(),
        profile.epoch(),
        std::time::Duration::from_secs(5),
    )?;
    let authenticator = AuthenticatorRef {
        id: profile.identity().authenticator,
        device_epoch: profile.epoch(),
    };
    let operation = carosign_linux::ids::KernelIds
        .new_operation_id()
        .map_err(|_| std::io::Error::other("нет ID"))?;
    let set = profile
        .identity()
        .resource_set(authenticator)
        .map_err(|_| std::io::Error::other("нет ресурса"))?;
    let lease = futures_executor::block_on(resources.acquire_for(
        ResourceOwner {
            operation,
            worker: supervisor.worker_id(),
            authenticator,
        },
        &set,
    ))
    .map_err(|_| std::io::Error::other("ресурс недоступен"))?;
    let scope = crate::resources::CleanupScope::of(&lease);
    let probe = crate::supervised_verification::prepare_mutation_tunnel(
        &mut supervisor,
        resources,
        &scope,
        *profile.identity().package.as_bytes(),
        *profile.identity().profile.as_bytes(),
        std::time::Duration::from_secs(5),
        false,
    );
    let cleanup = crate::resources::SupervisorCleanup::new_for_scope(
        supervisor,
        Arc::new(resources.clone()),
        scope.clone(),
    );
    let scheduler = resources.scheduler();
    let held = scheduler
        .lock()
        .map_err(|_| std::io::Error::other("планировщик недоступен"))?
        .begin_quiescing(scope.lease_id(), scope.owner(), scope.resource_set())
        .ok_or_else(|| std::io::Error::other("выдача probe не найдена"))?;
    let proof = futures_executor::block_on(
        carosign_runtime::scheduler::cleanup::confirm_quiesced_for(&cleanup, &scope, &held),
    );
    let released = match proof {
        Ok(proof) => futures_executor::block_on(
            carosign_runtime::scheduler::cleanup::confirm_released(&cleanup, &scope, proof),
        )
        .inspect_err(|error| {
            eprintln!("carosignd: освобождение ресурса после проверки готовности: {error:?}");
        }),
        Err(error) => {
            eprintln!("carosignd: остановка ресурса после проверки готовности: {error:?}");
            Err(error)
        }
    };
    let mut scheduler = scheduler
        .lock()
        .map_err(|_| std::io::Error::other("планировщик недоступен"))?;
    let released = match released {
        Ok(released) => released,
        Err(_) => {
            scheduler.quiescing_uncertain(held);
            return Err(std::io::Error::other("уборка probe не подтверждена"));
        }
    };
    if let Err(held) = scheduler.finish_quiescing(held, released) {
        scheduler.quiescing_uncertain(held);
        return Err(std::io::Error::other("освобождение probe не подтверждено"));
    }
    let (_, capabilities, _) =
        probe.map_err(|_| std::io::Error::other("способности провайдера недоступны"))?;
    if capabilities.modality != profile.identity().modality as i32 {
        return Err(std::io::Error::other("вид устройства не совпал с профилем"));
    }
    Ok(capabilities)
}
