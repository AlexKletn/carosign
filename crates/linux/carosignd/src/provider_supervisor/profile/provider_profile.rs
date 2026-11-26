use super::{
    Path, PathBuf, PolicyWatchState, ProtectedPolicySnapshot, ProviderIdentity, SystemResource, io,
    parse_extra_resource, parse_id, protected_executable,
};

/// Secure profile of one system provider.
pub struct ProviderProfile {
    pub(crate) remote: Option<crate::provider_catalog::RemoteProvider>,
    pub(in super::super) broker: PathBuf,
    pub(in super::super) provider: PathBuf,
    pub(in super::super) resource_owner: Option<PathBuf>,
    pub(in super::super) resource_recovery_owner: Option<PathBuf>,
    pub(in super::super) exclusive_resource_owner: bool,
    pub(in super::super) owner_managed_resource: Option<[u8; 16]>,
    pub(in super::super) enrollment_owner_endpoint: bool,
    pub(in super::super) owner_worker_endpoint: bool,
    pub(in super::super) enrollment_recipient:
        Option<carosign_runtime::enrollment_authorization::EnrollmentRecipientId>,
    pub(in super::super) provider_uid: u32,
    pub(in super::super) provider_gid: u32,
    pub(in super::super) identity: ProviderIdentity,
    pub(in super::super) resource_fd: Option<SystemResource>,
    pub(in super::super) additional_resources: Vec<SystemResource>,
    #[cfg(test)]
    pub(in super::super) mock_scenario: Option<String>,
    #[cfg(test)]
    pub(in super::super) mock_state: Option<std::ffi::OsString>,
}

impl ProviderProfile {
    pub(crate) fn epoch(&self) -> carosign_core::device::DeviceEpoch {
        self.remote
            .as_ref()
            .map_or(carosign_core::device::DeviceEpoch::FIRST, |remote| {
                remote.epoch()
            })
    }

    pub(crate) fn can_probe_device(&self) -> bool {
        self.requires_durable_binding()
            && self.exclusive_resource_owner
            && self.resource_owner.is_some()
    }

    pub(crate) fn permits_rendezvous(
        &self,
        id: carosign_core::ids::AuthenticatorId,
        uid: u32,
        gid: u32,
    ) -> bool {
        self.identity.authenticator == id && (self.provider_uid, self.provider_gid) == (uid, gid)
    }

    pub(crate) fn recovery_owner(&self) -> Option<&Path> {
        self.resource_recovery_owner.as_deref()
    }
    #[cfg(test)]
    pub(crate) fn set_recovery_owner(&mut self, owner: PathBuf) {
        self.resource_owner = Some(owner.clone());
        self.exclusive_resource_owner = true;
        self.resource_recovery_owner = Some(owner);
    }

    /// A secure profile must explicitly include all required channels.
    pub(crate) fn challenged_owner_configured(&self) -> bool {
        self.exclusive_resource_owner
            && self.resource_owner.is_some()
            && self.enrollment_owner_endpoint
            && self.owner_worker_endpoint
            && self.enrollment_recipient.is_some()
    }
    pub(crate) fn enrollment_recipient(
        &self,
    ) -> Option<carosign_runtime::enrollment_authorization::EnrollmentRecipientId> {
        self.enrollment_recipient
    }
    #[cfg(test)]
    pub(crate) fn device_fixture(broker: PathBuf, provider: PathBuf) -> Self {
        let mut profile = Self::fixture(broker, provider);
        profile.resource_fd = Some(SystemResource {
            path: "/dev/null".into(),
            role: carosign_protocol::carosign::provider::v1::FdRole::Device,
            id: [7; 16],
        });
        profile
    }

    #[cfg(test)]
    pub(crate) fn fixture(broker: PathBuf, provider: PathBuf) -> Self {
        Self {
            remote: None,
            broker,
            provider,
            resource_owner: None,
            resource_recovery_owner: None,
            exclusive_resource_owner: false,
            owner_managed_resource: None,
            enrollment_owner_endpoint: false,
            owner_worker_endpoint: false,
            enrollment_recipient: None,
            provider_uid: unsafe { libc::getuid() },
            provider_gid: unsafe { libc::getgid() },
            identity: ProviderIdentity {
                authenticator: carosign_core::ids::AuthenticatorId::from_bytes([5; 16]).unwrap(),
                package: carosign_core::ids::PackageId::from_bytes([2; 16]).unwrap(),
                profile: carosign_core::ids::ProfileId::from_bytes([3; 16]).unwrap(),
                resources: carosign_core::resource::ResourceSet::new(vec![
                    carosign_core::resource::ResourceClaim {
                        id: carosign_core::ids::ResourceId::from_bytes([7; 16]).unwrap(),
                        mode: carosign_core::resource::AccessMode::Exclusive,
                    },
                ])
                .unwrap(),
                modality: carosign_protocol::carosign::common::v1::Modality::Fingerprint,
            },
            resource_fd: std::env::var_os("CAROSIGN_TEST_RESOURCE_PATH").map(|path| {
                SystemResource {
                    path: path.into(),
                    role: carosign_protocol::carosign::provider::v1::FdRole::Config,
                    id: [7; 16],
                }
            }),
            additional_resources: Vec::new(),
            mock_scenario: std::env::var("CAROSIGN_TEST_SCENARIO").ok(),
            mock_state: std::env::var_os("CAROSIGN_MOCK_STATE"),
        }
    }
    /// Read root-owned profile with tracks and UID/GID of unprivileged worker.
    pub fn open(path: &Path) -> io::Result<Self> {
        Self::open_watched(path).map(|(profile, _)| profile)
    }

    pub(crate) fn open_watched(
        path: &Path,
    ) -> io::Result<(Self, carosign_linux::policy::PolicyChangeWatch)> {
        let (bytes, mut watch) = ProtectedPolicySnapshot::open(path)?.into_parts();
        let content = std::str::from_utf8(&bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "не UTF-8"))?;
        let mut broker = None;
        let mut provider = None;
        let mut resource_owner = None;
        let mut resource_recovery_owner = None;
        let mut exclusive_resource_owner = None;
        let mut owner_managed = false;
        let mut enrollment_owner_endpoint = None;
        let mut owner_worker_endpoint = None;
        let mut enrollment_recipient = None;
        let mut provider_uid = None;
        let mut provider_gid = None;
        let mut authenticator = None;
        let mut package = None;
        let mut profile_id = None;
        let mut resource = None;
        let mut modality = None;
        let mut resource_path: Option<PathBuf> = None;
        let mut resource_role = None;
        let mut additional_resources = Vec::new();
        for line in content.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "неверное поле"))?;
            if key.trim() == "resource_extra" {
                additional_resources.push(parse_extra_resource(value.trim())?);
                continue;
            }
            if key.trim() == "resource_mode" {
                if owner_managed || value.trim() != "owner_managed" {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid resource mode",
                    ));
                }
                owner_managed = true;
                continue;
            }
            if key.trim() == "resource_owner_contract" {
                if exclusive_resource_owner.is_some() || value.trim() != "exclusive_quiescence_v1" {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "неверный договор владельца",
                    ));
                }
                exclusive_resource_owner = Some(true);
                continue;
            }
            if key.trim() == "resource_owner_enrollment_contract" {
                if enrollment_owner_endpoint.is_some() || value.trim() != "private_channel_v1" {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "неверный договор канала регистрации",
                    ));
                }
                enrollment_owner_endpoint = Some(true);
                continue;
            }
            if key.trim() == "resource_owner_worker_channel" {
                if owner_worker_endpoint.is_some() || value.trim() != "private_channel_v1" {
                    return Err(io::Error::other("неверный канал работника владельца"));
                }
                owner_worker_endpoint = Some(true);
                continue;
            }
            if key.trim() == "enrollment_recipient" {
                if enrollment_recipient.is_some() {
                    return Err(io::Error::other("повтор получателя регистрации"));
                }
                enrollment_recipient = Some(
                    carosign_runtime::enrollment_authorization::EnrollmentRecipientId::from_bytes(
                        parse_id(value.trim())?,
                    )
                    .map_err(|_| io::Error::other("неверный получатель регистрации"))?,
                );
                continue;
            }
            if key.trim() == "resource_path" {
                let path = PathBuf::from(value.trim());
                if resource_path.is_some()
                    || !path.is_absolute()
                    || path.components().any(|c| {
                        matches!(
                            c,
                            std::path::Component::ParentDir | std::path::Component::CurDir
                        )
                    })
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "неверный путь ресурса",
                    ));
                }
                resource_path = Some(path);
                continue;
            }
            if key.trim() == "resource_role" {
                if resource_role.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "повтор роли ресурса",
                    ));
                }
                resource_role = Some(match value.trim() {
                    "device" => carosign_protocol::carosign::provider::v1::FdRole::Device,
                    "config" => carosign_protocol::carosign::provider::v1::FdRole::Config,
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "неверная роль ресурса",
                        ));
                    }
                });
                continue;
            }
            if matches!(
                key.trim(),
                "authenticator" | "package" | "profile" | "resource"
            ) {
                let target = match key.trim() {
                    "authenticator" => &mut authenticator,
                    "package" => &mut package,
                    "profile" => &mut profile_id,
                    _ => &mut resource,
                };
                if target.is_some() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "повтор поля"));
                }
                *target = Some(parse_id(value.trim())?);
                continue;
            }
            if key.trim() == "modality" {
                if modality.is_some() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "повтор поля"));
                }
                modality = Some(match value.trim() {
                    "fingerprint" => carosign_protocol::carosign::common::v1::Modality::Fingerprint,
                    "face" => carosign_protocol::carosign::common::v1::Modality::Face,
                    "iris" => carosign_protocol::carosign::common::v1::Modality::Iris,
                    "voice" => carosign_protocol::carosign::common::v1::Modality::Voice,
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "неизвестный вид",
                        ));
                    }
                });
                continue;
            }
            if matches!(key.trim(), "provider_uid" | "provider_gid") {
                let target = if key.trim() == "provider_uid" {
                    &mut provider_uid
                } else {
                    &mut provider_gid
                };
                if target.is_some() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "повтор поля"));
                }
                let id = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "неверный UID/GID"))?;
                if id == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "работник не должен быть root",
                    ));
                }
                *target = Some(id);
                continue;
            }
            let target = match key.trim() {
                "broker" => &mut broker,
                "provider" => &mut provider,
                "resource_owner" => &mut resource_owner,
                "resource_recovery_owner" => &mut resource_recovery_owner,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "неизвестное поле",
                    ));
                }
            };
            if target.is_some() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "повтор поля"));
            }
            *target = Some(protected_executable(Path::new(value.trim()))?);
        }
        if watch.check()? != PolicyWatchState::Unchanged {
            return Err(io::Error::other("профиль изменился"));
        }
        let resource_fd = match (resource_path, resource_role) {
            (None, None) => None,
            (Some(path), Some(role)) => Some(SystemResource {
                path,
                role,
                id: resource
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "нет resource"))?,
            }),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "неполный ресурс",
                ));
            }
        };
        if owner_managed
            && (resource_fd.is_some()
                || resource_owner.is_none()
                || exclusive_resource_owner != Some(true)
                || additional_resources.iter().any(|entry| {
                    entry.role != carosign_protocol::carosign::provider::v1::FdRole::Config
                }))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid owner-managed resource",
            ));
        }
        if !owner_managed && !additional_resources.is_empty() && resource_fd.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "нет основного дескриптора ресурса",
            ));
        }
        if !owner_managed && resource_owner.is_some() && resource_fd.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "нет ресурсов владельца",
            ));
        }
        if exclusive_resource_owner == Some(true) && resource_owner.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "нет доверенного владельца",
            ));
        }
        if enrollment_owner_endpoint == Some(true)
            && (resource_owner.is_none()
                || exclusive_resource_owner != Some(true)
                || enrollment_recipient.is_none())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "каналу регистрации нужен исключительный владелец",
            ));
        }
        if enrollment_recipient.is_some() && enrollment_owner_endpoint != Some(true) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "получатель без канала регистрации",
            ));
        }
        if owner_worker_endpoint == Some(true)
            && (resource_owner.is_none() || exclusive_resource_owner != Some(true))
        {
            return Err(io::Error::other(
                "каналу работника нужен исключительный владелец",
            ));
        }
        let mut claims = vec![carosign_core::resource::ResourceClaim {
            id: carosign_core::ids::ResourceId::from_bytes(
                resource
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "нет resource"))?,
            )
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "неверный resource"))?,
            mode: carosign_core::resource::AccessMode::Exclusive,
        }];
        for entry in &additional_resources {
            claims.push(carosign_core::resource::ResourceClaim {
                id: carosign_core::ids::ResourceId::from_bytes(entry.id)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "неверный resource"))?,
                mode: carosign_core::resource::AccessMode::Exclusive,
            });
        }
        let resource_set = carosign_core::resource::ResourceSet::new(claims)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "неверные ресурсы"))?;
        Ok((
            Self {
                remote: None,
                broker: broker
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "нет broker"))?,
                provider: provider
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "нет provider"))?,
                resource_owner,
                resource_recovery_owner,
                exclusive_resource_owner: exclusive_resource_owner.unwrap_or(false),
                owner_managed_resource: resource.filter(|_| owner_managed),
                enrollment_owner_endpoint: enrollment_owner_endpoint.unwrap_or(false),
                owner_worker_endpoint: owner_worker_endpoint.unwrap_or(false),
                enrollment_recipient,
                provider_uid: provider_uid.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "нет provider_uid")
                })?,
                provider_gid: provider_gid.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "нет provider_gid")
                })?,
                identity: ProviderIdentity {
                    authenticator: carosign_core::ids::AuthenticatorId::from_bytes(
                        authenticator.ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "нет authenticator")
                        })?,
                    )
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "неверный authenticator")
                    })?,
                    package: carosign_core::ids::PackageId::from_bytes(package.ok_or_else(
                        || io::Error::new(io::ErrorKind::InvalidData, "нет package"),
                    )?)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "неверный package"))?,
                    profile: carosign_core::ids::ProfileId::from_bytes(profile_id.ok_or_else(
                        || io::Error::new(io::ErrorKind::InvalidData, "нет profile"),
                    )?)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "неверный profile"))?,
                    resources: resource_set,
                    modality: modality.ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "нет modality")
                    })?,
                },
                resource_fd,
                additional_resources,
                #[cfg(test)]
                mock_scenario: None,
                #[cfg(test)]
                mock_state: None,
            },
            watch,
        ))
    }

    /// System announcement of one device of this profile.
    pub fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    pub(in super::super) fn owner_resources(&self) -> impl Iterator<Item = [u8; 16]> {
        let has_device = self.has_device_resources();
        self.owner_managed_resource.into_iter().chain(
            self.resource_fd
                .iter()
                .chain(&self.additional_resources)
                .filter(move |resource| {
                    self.owner_managed_resource.is_none()
                        && (!has_device
                            || resource.role
                                == carosign_protocol::carosign::provider::v1::FdRole::Device)
                })
                .map(|resource| resource.id),
        )
    }

    pub(crate) fn requires_durable_binding(&self) -> bool {
        self.owner_managed_resource.is_some() || self.has_device_resources()
    }

    fn has_device_resources(&self) -> bool {
        use carosign_protocol::carosign::provider::v1::FdRole;
        self.resource_fd
            .iter()
            .chain(&self.additional_resources)
            .any(|resource| resource.role == FdRole::Device)
    }

    #[cfg(test)]
    pub(crate) fn has_exclusive_owner_contract(&self) -> bool {
        self.exclusive_resource_owner && self.resource_owner.is_some()
    }
}
