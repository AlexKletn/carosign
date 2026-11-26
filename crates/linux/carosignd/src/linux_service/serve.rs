use super::{
    Arc, CLIENT_WORKERS, CapabilityState, LinuxHealth, MutationActivity, TimeSource,
    VerificationOnly, device_readiness, probe_capabilities, probe_declaration,
    publish_client_socket, restore_resource_owner,
};

/// Accepts connections after restoring and loading system policy.
pub fn serve<S, T>(
    config: &crate::config::Config,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    profile: &crate::provider_supervisor::ProviderProfile,
    clock: Arc<T>,
) -> std::io::Result<()>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    serve_mode(config, store, policy, Some(profile), clock)
}

pub fn serve_discovered<S, T>(
    config: &crate::config::Config,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    clock: Arc<T>,
) -> std::io::Result<()>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    serve_mode(config, store, policy, None, clock)
}

fn serve_mode<S, T>(
    config: &crate::config::Config,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    profile: Option<&crate::provider_supervisor::ProviderProfile>,
    clock: Arc<T>,
) -> std::io::Result<()>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    // Fences are repaired before resources and anchorage are requested.
    // The missing or corrupted file is not an empty database.
    let resources = Arc::new(restore_resource_owner(
        config.resource_fence_store.as_deref(),
    )?);
    if let Some(profile) = profile
        && let Err(error) = super::recovered_fence::recover(profile, &resources, clock.boot_id())
    {
        eprintln!("carosignd: восстановление ресурса: {error}");
    }
    let blocked = carosign_persistence::mutation_store::recover_unarmed_v4(store, |operation| {
        resources.startup_cleanup_confirmed(operation)
    })
    .map_err(|error| std::io::Error::other(format!("восстановление журнала: {error:?}")))?;
    if blocked != 0 {
        eprintln!("carosignd: {blocked} областей ожидают сверки; они останутся заблокированными");
    }
    let capabilities = std::sync::Mutex::new(
        if let Some(profile) = profile.filter(|profile| profile.requires_durable_binding()) {
            device_readiness(probe_declaration(profile, clock.as_ref()).ok(), || {
                probe_capabilities(profile, resources.as_ref(), clock.as_ref())
            })
        } else if let Some(profile) = profile {
            let descriptor = probe_capabilities(profile, resources.as_ref(), clock.as_ref()).ok();
            CapabilityState {
                verified_ready: descriptor.is_some(),
                descriptor,
            }
        } else {
            CapabilityState {
                descriptor: None,
                verified_ready: false,
            }
        },
    );
    let activity = MutationActivity::default();
    let queue =
        crate::linux_dispatch::DeviceQueue::new(usize::from(config.queued_per_authenticator));
    let operations = super::operation_directory::OperationDirectory::default();
    let catalog = Arc::new(crate::provider_catalog::ProviderCatalog::new(
        config.resource_fence_store.is_some(),
    ));
    let _catalog_monitor = if profile.is_none() {
        Some(crate::provider_catalog::CatalogMonitor::start(
            catalog.clone(),
        )?)
    } else {
        None
    };
    let _device_readiness = if profile.is_none() && config.resource_fence_store.is_some() {
        let runtime = std::path::Path::new(&config.socket)
            .parent()
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        Some(super::discovery_readiness::DiscoveryReadiness::start(
            catalog.clone(),
            resources.clone(),
            clock.clone(),
            runtime.to_owned(),
        )?)
    } else {
        None
    };
    let listener = publish_client_socket(&config.socket)?;
    std::thread::scope(|scope| {
        let (failed, failure) = std::sync::mpsc::channel();
        for _ in 0..CLIENT_WORKERS {
            let failed = failed.clone();
            let listener = &listener;
            let resources = resources.clone();
            let clock = clock.clone();
            let capabilities = &capabilities;
            let activity = &activity;
            let catalog = &catalog;
            let operations = &operations;
            let queue = &queue;
            scope.spawn(move || {
                let result = serve_worker(
                    listener,
                    config,
                    store,
                    policy,
                    profile,
                    clock,
                    resources,
                    capabilities,
                    activity,
                    catalog,
                    operations,
                    queue,
                );
                if let Err(error) = result {
                    let _ = failed.send(error);
                    // We wake up other workers waiting to accept.
                    use std::os::fd::AsRawFd;
                    // SAFETY: The listener belongs to the caller and lives to the end of the
                    // scope.
                    unsafe { libc::shutdown(listener.as_raw_fd(), libc::SHUT_RDWR) };
                }
            });
        }
        drop(failed);
        Err(failure
            .recv()
            .unwrap_or_else(|_| std::io::Error::other("работники службы завершились")))
    })
}

#[allow(clippy::too_many_arguments)]
fn serve_worker<S, T>(
    listener: &std::os::fd::OwnedFd,
    config: &crate::config::Config,
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    profile: Option<&crate::provider_supervisor::ProviderProfile>,
    clock: Arc<T>,
    resources: Arc<crate::resources::LocalResourceOwner>,
    capabilities: &std::sync::Mutex<CapabilityState>,
    activity: &MutationActivity,
    catalog: &crate::provider_catalog::ProviderCatalog,
    operations: &super::operation_directory::OperationDirectory,
    queue: &crate::linux_dispatch::DeviceQueue,
) -> std::io::Result<()>
where
    S: carosign_persistence::store::MetadataStore,
    T: TimeSource + 'static,
{
    let ids = carosign_linux::ids::KernelIds;
    let unavailable = VerificationOnly;
    let no_resources = crate::provider_catalog::UnavailableResources;
    let asked = crate::talk::Asked {
        subject: carosign_core::identity::SubjectRef {
            authority: carosign_core::ids::AuthorityId::from_bytes(*b"linux-local-v001")
                .expect("идентификатор"),
            subject: carosign_adapters::policy::subject_of(0),
        },
        epoch: carosign_core::device::DeviceEpoch::FIRST,
    };
    loop {
        // The PAM call identifier is limited to the connection; completed sessions should not
        // exhaust the current process table through 256 inputs.
        // The general scheduler is saved and, with uncertain cleaning, holds the resource.
        let registry = Arc::new(crate::operations::OperationRegistry::default());
        operations.add(&registry);
        let control = Arc::new(crate::control_watch::ControlOwner::default());
        let challenges = crate::scoped_challenge::ScopedChallenges::new(4096);
        let service = crate::service::Service {
            store,
            authority: policy,
            ids: &ids,
            clock: clock.as_ref(),
            resources: resources.as_ref(),
            resource_sets: profile.map_or(
                &no_resources as &dyn crate::resources::ResourceSetSource,
                |profile| profile.identity(),
            ),
            operations: registry.as_ref(),
            provider: &unavailable,
            mutation: &unavailable,
        };
        let legacy_dispatcher = profile.map(|profile| crate::linux_dispatch::LinuxDispatcher {
            service: &service,
            profile,
            registry: registry.clone(),
            resources: resources.clone(),
            clock: clock.clone(),
            control: control.clone(),
            challenges: &challenges,
            capabilities,
            activity,
            queue,
        });
        let discovery = super::discovery_dispatcher::DiscoveryDispatcher {
            catalog,
            runtime: std::path::Path::new(&config.socket)
                .parent()
                .ok_or(std::io::ErrorKind::InvalidInput)?,
            service: &service,
            registry: &registry,
            resources: &resources,
            clock: &clock,
            control: &control,
            challenges: &challenges,
            capabilities,
            activity,
            queue,
        };
        let dispatcher: &dyn crate::talk::VerificationDispatcher = match &legacy_dispatcher {
            Some(legacy) => legacy,
            None => &discovery,
        };
        let dispatcher = super::observed_dispatcher::ObservedDispatcher {
            inner: dispatcher,
            operations: super::operation_status::OperationStatus {
                observer_limit: usize::from(config.observers_per_operation),
                directory: operations,
                policy,
                store,
            },
        };
        let health = LinuxHealth {
            store,
            policy,
            capabilities,
            activity,
        };
        let discovery_health = super::catalog_health::CatalogHealth {
            store,
            policy,
            catalog,
        };
        let health: &dyn crate::health::HealthSource = if profile.is_some() {
            &health
        } else {
            &discovery_health
        };
        match futures_executor::block_on(crate::talk::serve_counted_once_with_dispatcher(
            listener,
            &service,
            registry.clone(),
            asked,
            health,
            config,
            &dispatcher,
        )) {
            Ok(crate::talk::Ended::Refused(reason)) => {
                eprintln!("carosignd: сеанс отклонён: {reason}")
            }
            Ok(_) => {}
            Err(crate::talk::CountedServeError::Accept(
                crate::server::CountedAcceptError::Accept(crate::server::AcceptOutcome::Kernel(
                    libc::EINTR,
                )),
            )) => continue,
            Err(_) => return Err(std::io::Error::other("ошибка приёма соединения")),
        }
    }
}
