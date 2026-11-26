use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
    time::{Duration, Instant},
};

use super::RemoteProvider;
use crate::{config::DEFAULT_PROVIDER_ALLOWLIST, provider_supervisor::ProviderProfile};
use carosign_core::device::{AuthenticatorRef, DeviceEpoch};
use carosign_linux::policy::{PolicyWatchState, ProtectedPolicySnapshot};
use carosign_protocol::carosign::{
    client::v1::{Authenticators, ProviderInventory, ProviderPublication},
    common::v1::{AuthenticatorDescriptor, CapabilityDescriptor, UnavailabilityReason},
};

use super::{catalog_snapshot::CatalogSnapshot, published_device::PublishedDevice};

use super::{endpoint::describe, inventory_epochs::InventoryEpochs, publication::Publication};

const ROOT: &str = "/run/carosign/providers";

pub(crate) struct ProviderCatalog {
    readiness_enabled: bool,
    readiness: std::sync::Mutex<
        BTreeMap<
            carosign_core::ids::AuthenticatorId,
            super::device_readiness_proof::DeviceReadinessProof,
        >,
    >,
    epochs: std::sync::Arc<std::sync::Mutex<InventoryEpochs>>,
}

impl ProviderCatalog {
    pub(crate) fn new(readiness_enabled: bool) -> Self {
        Self {
            readiness_enabled,
            epochs: Default::default(),
            readiness: Default::default(),
        }
    }

    pub(super) fn publication_watch(
        &self,
    ) -> io::Result<super::publication_watch::PublicationWatch> {
        let mut watch = super::publication_watch::PublicationWatch::new()?;
        watch.add(Path::new("/run"), b"carosign")?;
        watch.add(Path::new("/run/carosign"), b"providers")?;
        watch.add(Path::new("/etc"), b"carosign")?;
        watch.add(Path::new("/etc/carosign"), b"providers.conf")?;
        let root = Path::new(ROOT);
        if let Ok(snapshot) = ProtectedPolicySnapshot::open(Path::new(DEFAULT_PROVIDER_ALLOWLIST)) {
            let (contents, _) = snapshot.into_parts();
            if let Ok(publications) = Self::parse(&contents) {
                for publication in publications {
                    watch.add(root, publication.name.as_bytes())?;
                    watch.add(&root.join(&publication.name), b"control.sock")?;
                }
            }
        }
        Ok(watch)
    }

    pub(super) fn invalidate_bindings(&self) {
        if let Ok(mut epochs) = self.epochs.lock() {
            epochs.invalidate_bindings();
        }
    }

    pub(crate) fn resolve(
        &self,
        id: carosign_core::ids::AuthenticatorId,
        runtime: &Path,
    ) -> io::Result<ProviderProfile> {
        let (contents, mut watch) =
            ProtectedPolicySnapshot::open(Path::new(DEFAULT_PROVIDER_ALLOWLIST))?.into_parts();
        let publication = Self::parse(&contents)?
            .into_iter()
            .find(|entry| entry.devices.contains_key(&id))
            .ok_or(io::ErrorKind::NotFound)?;
        Self::require_enabled(&publication, id)?;
        let (mut profile, profile_watch) = Self::load_profile(&publication, id)?;
        let mut epochs = self
            .epochs
            .lock()
            .map_err(|_| io::Error::other("инвентарь недоступен"))?;
        self.refresh(&mut epochs)?;
        let (identity, epoch) = epochs.binding(id).ok_or(io::ErrorKind::NotFound)?;
        if watch.check()? != PolicyWatchState::Unchanged {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        profile.remote = Some(RemoteProvider {
            publication,
            root: Path::new(ROOT),
            runtime: runtime.to_owned(),
            identity,
            authenticator: AuthenticatorRef {
                id,
                device_epoch: DeviceEpoch::new(epoch).map_err(|_| io::ErrorKind::InvalidData)?,
            },
            watches: std::sync::Mutex::new([watch, profile_watch]),
            epochs: self.epochs.clone(),
        });
        Ok(profile)
    }

    pub(crate) fn execution_profile(
        &self,
        id: carosign_core::ids::AuthenticatorId,
    ) -> io::Result<ProviderProfile> {
        let (contents, mut watch) =
            ProtectedPolicySnapshot::open(Path::new(DEFAULT_PROVIDER_ALLOWLIST))?.into_parts();
        let publication = Self::parse(&contents)?
            .into_iter()
            .find(|entry| entry.devices.contains_key(&id))
            .ok_or(io::ErrorKind::NotFound)?;
        let (profile, _) = Self::load_profile(&publication, id)?;
        if watch.check()? != PolicyWatchState::Unchanged {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(profile)
    }

    fn require_enabled(
        publication: &Publication,
        id: carosign_core::ids::AuthenticatorId,
    ) -> io::Result<()> {
        match publication.devices.get(&id) {
            Some(device) if device.disabled => Err(io::ErrorKind::PermissionDenied.into()),
            Some(_) => Ok(()),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }

    fn load_profile(
        publication: &Publication,
        id: carosign_core::ids::AuthenticatorId,
    ) -> io::Result<(ProviderProfile, carosign_linux::policy::PolicyChangeWatch)> {
        let path = publication
            .devices
            .get(&id)
            .and_then(|device| device.profile.as_deref())
            .ok_or(io::ErrorKind::NotFound)?;
        let (profile, watch) = ProviderProfile::open_watched(path)?;
        if !profile.permits_rendezvous(id, publication.uid, publication.gid) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok((profile, watch))
    }

    pub(crate) fn inventory(&self) -> io::Result<Vec<AuthenticatorDescriptor>> {
        self.public_inventory().map(|inventory| inventory.entries)
    }

    pub(crate) fn public_inventory(&self) -> io::Result<Authenticators> {
        let mut epochs = self
            .epochs
            .lock()
            .map_err(|_| io::Error::other("инвентарь недоступен"))?;
        self.refresh(&mut epochs).map(|snapshot| snapshot.inventory)
    }

    pub(crate) fn readiness_candidates(&self) -> io::Result<Vec<AuthenticatorRef>> {
        let mut epochs = self
            .epochs
            .lock()
            .map_err(|_| io::Error::other("инвентарь недоступен"))?;
        self.refresh(&mut epochs)
            .map(|snapshot| snapshot.candidates)
    }

    pub(crate) fn record_readiness(
        &self,
        profile: ProviderProfile,
        result: Result<CapabilityDescriptor, UnavailabilityReason>,
    ) {
        let Some(remote) = profile.remote else {
            return;
        };
        let Ok(epochs) = self.epochs.lock() else {
            return;
        };
        let id = remote.authenticator.id;
        let proof = super::device_readiness_proof::DeviceReadinessProof { remote, result };
        if proof.matches(epochs.binding(id))
            && let Ok(mut readiness) = self.readiness.lock()
        {
            readiness.insert(id, proof);
        }
    }

    fn refresh(&self, epochs: &mut InventoryEpochs) -> io::Result<CatalogSnapshot> {
        let (contents, mut watch) =
            match ProtectedPolicySnapshot::open(Path::new(DEFAULT_PROVIDER_ALLOWLIST)) {
                Ok(snapshot) => snapshot.into_parts(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    self.readiness
                        .lock()
                        .map_err(|_| io::Error::other("готовность недоступна"))?
                        .clear();
                    return Ok(CatalogSnapshot {
                        inventory: Authenticators {
                            entries: epochs.update(Vec::new(), [])?,
                            providers: Some(ProviderInventory::default()),
                        },
                        candidates: Vec::new(),
                    });
                }
                Err(error) => return Err(error),
            };
        let publications = Self::parse(&contents)?;
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut entries = Vec::new();
        let mut devices = BTreeSet::new();
        let mut status = Vec::with_capacity(publications.len());
        let mut unreachable = BTreeSet::new();
        for publication in &publications {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let response = describe(
                publication,
                Path::new(ROOT),
                remaining.min(Duration::from_millis(15)),
            );
            let reachable = response.is_ok();
            if !reachable {
                unreachable.extend(publication.devices.keys().copied());
            }
            if let Ok((descriptors, identity)) = response {
                for mut descriptor in descriptors {
                    let id = carosign_core::ids::AuthenticatorId::try_from(
                        descriptor.authenticator_id.as_slice(),
                    )
                    .map_err(|_| io::ErrorKind::InvalidData)?;
                    let reason = self.classify_device(publication, id, &mut descriptor);
                    descriptor.available = reason == UnavailabilityReason::Unspecified;
                    descriptor.unavailability_reason = reason as i32;
                    if reason == UnavailabilityReason::ReadinessCheckPending {
                        devices.insert(id);
                    }
                    if let Some(capabilities) = descriptor.capabilities.as_mut() {
                        carosign_protocol::validation::normalize_capture_metadata(capabilities);
                    }
                    entries.push((descriptor, identity));
                }
            }
            status.push(ProviderPublication {
                name: publication.name.clone(),
                reachable,
                authenticator_ids: Vec::new(),
            });
        }
        if !matches!(watch.check()?, PolicyWatchState::Unchanged) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let mut inventory = epochs.update(
            entries,
            publications
                .iter()
                .flat_map(|entry| entry.devices.keys().copied()),
        )?;
        let mut readiness = self
            .readiness
            .lock()
            .map_err(|_| io::Error::other("готовность недоступна"))?;
        readiness.retain(|id, proof| devices.contains(id) && proof.matches(epochs.binding(*id)));
        let disabled = publications
            .iter()
            .flat_map(|publication| &publication.devices)
            .filter_map(|(&id, device)| device.disabled.then_some(id))
            .collect::<BTreeSet<_>>();
        let mut candidates = Vec::new();
        for descriptor in &mut inventory {
            let id = carosign_core::ids::AuthenticatorId::try_from(
                descriptor.authenticator_id.as_slice(),
            )
            .map_err(|_| io::ErrorKind::InvalidData)?;
            if disabled.contains(&id) {
                descriptor.available = false;
                descriptor.unavailability_reason =
                    UnavailabilityReason::DisabledByAdministrator as i32;
                continue;
            }
            if epochs.binding(id).is_none() {
                descriptor.unavailability_reason = if unreachable.contains(&id) {
                    UnavailabilityReason::ProviderUnreachable
                } else {
                    UnavailabilityReason::DeviceNotPublished
                } as i32;
            }
            let result = readiness.get(&id).map(|proof| &proof.result);
            if let Some(Ok(capabilities)) = result {
                descriptor.available = true;
                descriptor.unavailability_reason = UnavailabilityReason::Unspecified as i32;
                let mut capabilities = capabilities.clone();
                if let Some(declaration) = descriptor.capabilities.take() {
                    capabilities.assurance = capabilities.assurance.or(declaration.assurance);
                    capabilities.enrollment_stages = capabilities
                        .enrollment_stages
                        .or(declaration.enrollment_stages);
                }
                descriptor.capabilities = Some(capabilities);
            } else if devices.contains(&id) {
                if let Some(Err(reason)) = result {
                    descriptor.unavailability_reason = *reason as i32;
                }
                candidates.push(AuthenticatorRef {
                    id,
                    device_epoch: DeviceEpoch::new(descriptor.device_epoch)
                        .map_err(|_| io::ErrorKind::InvalidData)?,
                });
            }
        }
        for id in disabled {
            if !inventory
                .iter()
                .any(|descriptor| descriptor.authenticator_id == id.as_bytes().as_slice())
            {
                inventory.push(AuthenticatorDescriptor {
                    authenticator_id: id.as_bytes().to_vec(),
                    unavailability_reason: UnavailabilityReason::DisabledByAdministrator as i32,
                    ..Default::default()
                });
            }
        }
        for (publication, status) in publications.iter().zip(&mut status) {
            status
                .authenticator_ids
                .extend(publication.devices.keys().filter_map(|id| {
                    inventory.iter().find_map(|descriptor| {
                        (descriptor.authenticator_id == id.as_bytes().as_slice())
                            .then(|| descriptor.authenticator_id.clone())
                    })
                }));
        }
        Ok(CatalogSnapshot {
            inventory: Authenticators {
                entries: inventory,
                providers: Some(ProviderInventory {
                    publications: status,
                }),
            },
            candidates,
        })
    }

    fn classify_device(
        &self,
        publication: &Publication,
        id: carosign_core::ids::AuthenticatorId,
        descriptor: &mut AuthenticatorDescriptor,
    ) -> UnavailabilityReason {
        use UnavailabilityReason::*;
        let Some(device) = publication.devices.get(&id) else {
            return ExecutionProfileRejected;
        };
        if device.disabled {
            return DisabledByAdministrator;
        }
        if device.profile.is_none() {
            return InventoryOnly;
        }
        let Ok((profile, _)) = Self::load_profile(publication, id) else {
            return ExecutionProfileRejected;
        };
        let modality = profile.identity().modality as i32;
        if !descriptor
            .capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.modality == modality)
        {
            descriptor.capabilities = Some(CapabilityDescriptor {
                modality,
                ..Default::default()
            });
            return ModalityMismatch;
        }
        if !descriptor.available {
            return ProviderReportedUnavailable;
        }
        if !crate::linux_dispatch::available_capabilities(descriptor.capabilities.as_ref(), true) {
            return UnsupportedCapabilities;
        }
        if !profile.requires_durable_binding() {
            return Unspecified;
        }
        if !self.readiness_enabled || !profile.can_probe_device() {
            return ReadinessCheckUnconfigured;
        }
        ReadinessCheckPending
    }

    pub(crate) fn validate_configuration(contents: &[u8]) -> io::Result<()> {
        for publication in Self::parse(contents)? {
            for (&id, device) in &publication.devices {
                if device.profile.is_some() {
                    Self::load_profile(&publication, id)?;
                }
            }
        }
        Ok(())
    }

    fn parse(contents: &[u8]) -> io::Result<Vec<Publication>> {
        let contents = std::str::from_utf8(contents).map_err(|_| io::ErrorKind::InvalidData)?;
        let mut result: Vec<Publication> = Vec::new();
        let mut uids = BTreeSet::new();
        let mut devices = BTreeSet::new();
        for line in contents
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let mut parts = line.split_ascii_whitespace();
            let (Some(name), Some(uid), Some(gid), Some(id), profile, flag, None) = (
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
                parts.next(),
            ) else {
                return Err(io::ErrorKind::InvalidData.into());
            };
            let uid = uid.parse::<u32>().map_err(|_| io::ErrorKind::InvalidData)?;
            let gid = gid.parse::<u32>().map_err(|_| io::ErrorKind::InvalidData)?;
            let id = carosign_core::ids::AuthenticatorId::from_bytes(
                crate::provider_supervisor::parse_id(id)?,
            )
            .map_err(|_| io::ErrorKind::InvalidData)?;
            let (profile, disabled) = match (profile, flag) {
                (None, None) => (None, false),
                (Some("disabled"), None) => (None, true),
                (Some(path), None) => (Some(path), false),
                (Some(path), Some("disabled")) => (Some(path), true),
                _ => return Err(io::ErrorKind::InvalidData.into()),
            };
            let profile = profile.map(std::path::PathBuf::from);
            if devices.len() >= 64
                || profile.as_ref().is_some_and(|path| !path.is_absolute())
                || name.len() > 48
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || uid == 0
                || gid == 0
                || !devices.insert(id)
            {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let device = PublishedDevice { profile, disabled };
            if let Some(publication) = result.iter_mut().find(|entry| entry.name == name) {
                if (publication.uid, publication.gid) != (uid, gid) {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                publication.devices.insert(id, device);
            } else {
                if !uids.insert(uid) {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                result.push(Publication {
                    name: name.to_owned(),
                    uid,
                    gid,
                    devices: [(id, device)].into(),
                });
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "catalog.tests.rs"]
mod tests;
