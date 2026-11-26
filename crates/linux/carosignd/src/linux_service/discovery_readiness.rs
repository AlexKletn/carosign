use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use carosign_protocol::carosign::common::v1::UnavailabilityReason;
use carosign_runtime::ports::TimeSource;

use crate::{
    linux_dispatch::available_capabilities, provider_catalog::ProviderCatalog,
    resources::LocalResourceOwner,
};

use super::{probe_capabilities, probe_declaration};

pub(super) struct DiscoveryReadiness {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl DiscoveryReadiness {
    pub(super) fn start<T: TimeSource + 'static>(
        catalog: Arc<ProviderCatalog>,
        resources: Arc<LocalResourceOwner>,
        clock: Arc<T>,
        runtime: PathBuf,
    ) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::Builder::new()
            .name("provider-readiness".into())
            .spawn(move || {
                let mut previous = None;
                let mut recovery_attempts = std::collections::BTreeMap::new();
                while !stopping.load(Ordering::Relaxed) {
                    if let Ok(candidates) = catalog.readiness_candidates()
                        && let Some(candidate) = candidates
                            .iter()
                            .filter(|candidate| previous.is_none_or(|id| candidate.id > id))
                            .min_by_key(|candidate| candidate.id)
                            .or_else(|| candidates.iter().min_by_key(|candidate| candidate.id))
                    {
                        previous = Some(candidate.id);
                        if let Ok(profile) = catalog.resolve(candidate.id, &runtime)
                            && profile.epoch() == candidate.device_epoch
                            && profile.can_probe_device()
                        {
                            let fenced =
                                resources.has_recovered_fence(&profile.identity().resources);
                            if matches!(fenced, Ok(true)) && profile.recovery_owner().is_some() {
                                let now = std::time::Instant::now();
                                let next = recovery_attempts.entry(candidate.id).or_insert(now);
                                if now >= *next {
                                    *next = now + Duration::from_secs(30);
                                    if let Err(error) = super::recovered_fence::recover(
                                        &profile,
                                        &resources,
                                        clock.boot_id(),
                                    ) {
                                        eprintln!("carosignd: восстановление ресурса: {error}");
                                    }
                                }
                            }
                            let result = match resources
                                .has_recovered_fence(&profile.identity().resources)
                            {
                                Ok(true) => Err(UnavailabilityReason::ResourceFenced),
                                Err(_) => Err(UnavailabilityReason::ReadinessCheckFailed),
                                Ok(false) => match probe_declaration(&profile, clock.as_ref()) {
                                    Err(_) => Err(UnavailabilityReason::ReadinessCheckFailed),
                                    Ok(declaration)
                                        if !available_capabilities(Some(&declaration), true) =>
                                    {
                                        Err(UnavailabilityReason::UnsupportedCapabilities)
                                    }
                                    Ok(_) => probe_capabilities(
                                        &profile,
                                        &resources,
                                        clock.as_ref(),
                                    )
                                    .map_err(|_| UnavailabilityReason::ReadinessCheckFailed)
                                    .and_then(|capabilities| {
                                        if available_capabilities(Some(&capabilities), true) {
                                            Ok(capabilities)
                                        } else {
                                            Err(UnavailabilityReason::UnsupportedCapabilities)
                                        }
                                    }),
                                },
                            };
                            catalog.record_readiness(profile, result);
                        }
                    }
                    if !stopping.load(Ordering::Relaxed) {
                        thread::park_timeout(Duration::from_secs(2));
                    }
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}

impl Drop for DiscoveryReadiness {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
