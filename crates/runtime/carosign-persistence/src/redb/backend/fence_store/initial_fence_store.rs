use super::{
    AUTHENTICATORS, Database, DurableReceipt, DurableReleasePermit, ENROLLMENT_INDEX, ENROLLMENTS,
    FenceScope, GuardedWriteTx, IDEMPOTENCY, INSTALLATIONS, META, MUTATIONS, OUTBOX, PACKAGES,
    PROVIDER_STATE, Path, PreflightBackend, RESOURCE_FENCE_VALUE, RESOURCE_FENCES, ReadTxRef,
    ReadableTable, ReadableTableMetadata, SCHEMA_KEY, SCHEMA_V5, SCOPE_LOCK, SCOPED_FENCE_MAGIC,
    SharedBackend, StorageError, TEMPLATE_BLOBS, TableHandle, WriteTransaction, WriteTx,
    scoped_fence_resources,
};

/// Isolated primary scheme for checking sustainable resource fencing.
/// Application records use the v4 format and undergo the same image checks; it cannot
/// completely replace the working storage and connect the service yet.
pub struct InitialFenceStore {
    pub(in super::super) database: Database,
    path: std::path::PathBuf,
    pub(super) device: u64,
    inode: u64,
}

impl InitialFenceStore {
    /// He reads exactly one complete fence from the operation, matching the fields doesn't
    /// confirm peace, and doesn't create an authority to remove the fence.
    pub fn inspect_recovery_scope(
        &self,
        operation: carosign_core::ids::OperationId,
    ) -> Result<FenceScope, StorageError> {
        use carosign_core::{
            device::{AuthenticatorRef, DeviceEpoch},
            ids::{AuthenticatorId, LeaseId, WorkerInstanceId},
            resource::{AccessMode, ResourceClaim, ResourceSet},
        };
        self.check_identity()?;
        let tx = self
            .database
            .begin_read()
            .map_err(|_| StorageError::Unavailable)?;
        let table = tx
            .open_table(RESOURCE_FENCES)
            .map_err(|_| StorageError::Unavailable)?;
        let mut found: Option<Vec<u8>> = None;
        for entry in table.iter().map_err(|_| StorageError::Unavailable)? {
            let (_, value) = entry.map_err(|_| StorageError::Unavailable)?;
            let bytes = value.value();
            if bytes.len() < 77
                || &bytes[..4] != SCOPED_FENCE_MAGIC
                || bytes[20..36] != *operation.as_bytes()
            {
                continue;
            }
            if scoped_fence_resources(bytes).is_none()
                || found.as_ref().is_some_and(|other| other != bytes)
            {
                return Err(StorageError::Corrupt {
                    what: "неоднозначное ограждение операции",
                });
            }
            found.get_or_insert_with(|| bytes.to_vec());
        }
        let bytes = found.ok_or(StorageError::Unavailable)?;
        let arr = |offset| -> [u8; 16] {
            bytes[offset..offset + 16]
                .try_into()
                .expect("проверенная длина")
        };
        let lease = LeaseId::from_bytes(arr(4)).map_err(|_| StorageError::Unavailable)?;
        let worker =
            WorkerInstanceId::from_bytes(arr(36)).map_err(|_| StorageError::Unavailable)?;
        let authenticator =
            AuthenticatorId::from_bytes(arr(52)).map_err(|_| StorageError::Unavailable)?;
        let epoch = DeviceEpoch::new(u64::from_le_bytes(
            bytes[68..76].try_into().expect("проверенная длина"),
        ))
        .map_err(|_| StorageError::Unavailable)?;
        let mut claims = Vec::new();
        for chunk in bytes[77..].chunks_exact(17) {
            let id = carosign_core::ids::ResourceId::from_bytes(
                chunk[..16].try_into().expect("длина записи"),
            )
            .map_err(|_| StorageError::Unavailable)?;
            let mode = match chunk[16] {
                1 => AccessMode::Exclusive,
                2 => AccessMode::SharedRead,
                3 => AccessMode::Mediated,
                _ => return Err(StorageError::Unavailable),
            };
            if table
                .get(id.as_bytes().as_slice())
                .map_err(|_| StorageError::Unavailable)?
                .is_none_or(|value| value.value() != bytes)
            {
                return Err(StorageError::Corrupt {
                    what: "неполный набор ограждения",
                });
            }
            claims.push(ResourceClaim { id, mode });
        }
        let resources = ResourceSet::new(claims).map_err(|_| StorageError::Unavailable)?;
        self.check_identity()?;
        Ok(FenceScope {
            lease,
            operation,
            worker,
            authenticator: AuthenticatorRef {
                id: authenticator,
                device_epoch: epoch,
            },
            resources,
        })
    }
    /// Creates only a new file; the existing database does not open or change.
    /// The catalogue and all its ancestors must be protected from being replaced by others.
    pub fn create_new(path: &Path) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
        if !path.is_absolute() {
            return Err(StorageError::Unavailable);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| StorageError::Unavailable)?;
        use std::os::unix::fs::MetadataExt;
        let identity = file.metadata().map_err(|_| StorageError::Unavailable)?;
        let database = Database::builder()
            .create_file(file)
            .map_err(|_| StorageError::Unavailable)?;
        let tx = database
            .begin_write()
            .map_err(|_| StorageError::Unavailable)?;
        {
            for definition in [
                META,
                AUTHENTICATORS,
                ENROLLMENTS,
                ENROLLMENT_INDEX,
                MUTATIONS,
                SCOPE_LOCK,
                OUTBOX,
                IDEMPOTENCY,
                PROVIDER_STATE,
                PACKAGES,
                TEMPLATE_BLOBS,
                INSTALLATIONS,
                RESOURCE_FENCES,
            ] {
                tx.open_table(definition)
                    .map_err(|_| StorageError::Unavailable)?;
            }
            tx.open_table(META)
                .map_err(|_| StorageError::Unavailable)?
                .insert(SCHEMA_KEY, SCHEMA_V5)
                .map_err(|_| StorageError::Unavailable)?;
        }
        tx.commit().map_err(|_| StorageError::Unavailable)?;
        std::fs::File::open(path.parent().ok_or(StorageError::Unavailable)?)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| StorageError::Unavailable)?;
        let store = Self {
            database,
            path: path.to_owned(),
            device: identity.dev(),
            inode: identity.ino(),
        };
        store.check_identity()?;
        Ok(store)
    }

    /// Checks the image in memory before opening the file for recording; v4 does not change.
    /// The catalogue and all its ancestors must be protected from being replaced by others.
    pub fn open_guarded(path: &Path) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
        if !path.is_absolute() {
            return Err(StorageError::Unavailable);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| StorageError::Unavailable)?;
        let metadata = file.metadata().map_err(|_| StorageError::Unavailable)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(StorageError::Corrupt {
                what: "пустая база",
            });
        }
        use std::os::unix::fs::MetadataExt;
        let (device, inode) = (metadata.dev(), metadata.ino());
        let base = std::sync::Arc::new(
            redb::backends::FileBackend::new(file).map_err(|_| StorageError::Unavailable)?,
        );
        let overlay = PreflightBackend::new(std::sync::Arc::clone(&base))
            .map_err(|_| StorageError::Unavailable)?;
        {
            let probe = Database::builder()
                .create_with_backend(overlay)
                .map_err(|_| StorageError::Corrupt {
                    what: "база v5 не открывается",
                })?;
            let tx = probe.begin_read().map_err(|_| StorageError::Unavailable)?;
            let names = [
                "meta",
                "authenticators",
                "enrollments",
                "enrollment_index",
                "mutations",
                "mutation_scope_lock",
                "effect_outbox",
                "idempotency",
                "provider_state",
                "packages",
                "template_blobs",
                "provider_installations",
                "resource_fences",
            ];
            let tables: Vec<_> = tx
                .list_tables()
                .map_err(|_| StorageError::Unavailable)?
                .collect();
            if tables.len() != names.len()
                || tables.iter().any(|t| !names.contains(&t.name()))
                || tx
                    .list_multimap_tables()
                    .map_err(|_| StorageError::Unavailable)?
                    .next()
                    .is_some()
            {
                return Err(StorageError::Corrupt {
                    what: "неизвестный состав таблиц v5",
                });
            }
            let meta = tx.open_table(META).map_err(|_| StorageError::Unavailable)?;
            if meta.len().map_err(|_| StorageError::Unavailable)? != 1
                || meta
                    .get(SCHEMA_KEY)
                    .map_err(|_| StorageError::Unavailable)?
                    .is_none_or(|v| v.value() != SCHEMA_V5)
            {
                return Err(StorageError::Corrupt {
                    what: "неизвестная схема v5",
                });
            }
            for definition in [AUTHENTICATORS, PROVIDER_STATE, TEMPLATE_BLOBS] {
                let table = tx
                    .open_table(definition)
                    .map_err(|_| StorageError::Unavailable)?;
                if !table.is_empty().map_err(|_| StorageError::Unavailable)? {
                    return Err(StorageError::Corrupt {
                        what: "прикладные таблицы v5 ещё не поддерживаются",
                    });
                }
            }
            // The base version adds fencing; the format of the remaining records remains v4.
            // We perform the same link checks as when you open v4.
            crate::installation_records::validate_v4_snapshot(&tx).map_err(|_| {
                StorageError::Corrupt {
                    what: "несогласованные пакеты или установки v5",
                }
            })?;
            crate::mutation_store::scan_prepared_v4_snapshot(&ReadTxRef { tx: &tx })?;
            let fences = tx
                .open_table(RESOURCE_FENCES)
                .map_err(|_| StorageError::Unavailable)?;
            for entry in fences.iter().map_err(|_| StorageError::Unavailable)? {
                let (key, value) = entry.map_err(|_| StorageError::Unavailable)?;
                if key.value().len() != 16 || key.value().iter().all(|&b| b == 0) {
                    return Err(StorageError::Corrupt {
                        what: "неверное ограждение ресурса",
                    });
                }
                if value.value() != RESOURCE_FENCE_VALUE {
                    let ids =
                        scoped_fence_resources(value.value()).ok_or(StorageError::Corrupt {
                            what: "неверный контекст ограждения",
                        })?;
                    if !ids.iter().any(|id| id.as_slice() == key.value()) {
                        return Err(StorageError::Corrupt {
                            what: "ресурс отсутствует в контексте ограждения",
                        });
                    }
                    for id in ids {
                        if fences
                            .get(id.as_slice())
                            .map_err(|_| StorageError::Unavailable)?
                            .is_none_or(|other| other.value() != value.value())
                        {
                            return Err(StorageError::Corrupt {
                                what: "неполный набор ограждения",
                            });
                        }
                    }
                }
            }
        }
        let database = Database::builder()
            .create_with_backend(SharedBackend(base))
            .map_err(|_| StorageError::Unavailable)?;
        let store = Self {
            database,
            path: path.to_owned(),
            device,
            inode,
        };
        store.check_identity()?;
        Ok(store)
    }

    pub(in super::super) fn check_identity(&self) -> Result<(), StorageError> {
        use std::os::unix::fs::MetadataExt;
        let metadata =
            std::fs::symlink_metadata(&self.path).map_err(|_| StorageError::Unavailable)?;
        if !metadata.is_file() || metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(StorageError::Unavailable);
        }
        Ok(())
    }

    /// Stable encloses a resource; a failure does not prove the absence of a record.
    pub fn fence(
        &self,
        resource: carosign_core::ids::ResourceId,
    ) -> Result<DurableReceipt, StorageError> {
        self.fence_resources(&[resource])
    }

    /// Sustainablely encloses the entire set with one transaction before transferring FD.
    /// Empty, repetitive, too large or already fenced set is rejected, if the error result of
    /// the fixation may be unknown: transmission is prohibited.
    pub fn fence_resources(
        &self,
        resources: &[carosign_core::ids::ResourceId],
    ) -> Result<DurableReceipt, StorageError> {
        if resources.is_empty()
            || resources.len() > carosign_core::resource::RESOURCE_SET_MAX
            || resources
                .iter()
                .enumerate()
                .any(|(index, id)| resources[..index].contains(id))
        {
            return Err(StorageError::Unavailable);
        }
        self.write_fences(resources, RESOURCE_FENCE_VALUE)
    }

    /// Sustainablely connects the whole set with a specific issue and owner.
    /// Context is not proof that the device is finished.
    pub fn fence_scoped(&self, scope: &FenceScope) -> Result<DurableReceipt, StorageError> {
        let resources: Vec<_> = scope
            .resources
            .claims()
            .iter()
            .map(|claim| claim.id)
            .collect();
        self.write_fences(&resources, &scope.encode())
    }

    /// It removes the confirmed issuance with one stable transaction.
    /// In case of an error, confirmation is not issued: the result of the fixation may be
    /// unknown, and the caller is obliged to leave the operational fence.
    pub fn release_verified(
        &self,
        permit: DurableReleasePermit,
    ) -> Result<DurableReceipt, StorageError> {
        let expected = permit.0.encode();
        let tx = self.begin_guarded_write()?;
        {
            let mut table =
                tx.0.tx
                    .as_ref()
                    .ok_or(StorageError::Unavailable)?
                    .open_table(RESOURCE_FENCES)
                    .map_err(|_| StorageError::Unavailable)?;
            for claim in permit.0.resources.claims() {
                if table
                    .get(claim.id.as_bytes().as_slice())
                    .map_err(|_| StorageError::Unavailable)?
                    .is_none_or(|value| value.value() != expected.as_slice())
                {
                    return Err(StorageError::Unavailable);
                }
            }
            for claim in permit.0.resources.claims() {
                table
                    .remove(claim.id.as_bytes().as_slice())
                    .map_err(|_| StorageError::Unavailable)?;
            }
        }
        Box::new(tx).commit_durable()
    }

    fn write_fences(
        &self,
        resources: &[carosign_core::ids::ResourceId],
        value: &[u8],
    ) -> Result<DurableReceipt, StorageError> {
        let tx = self.begin_guarded_write()?;
        {
            let mut table =
                tx.0.tx
                    .as_ref()
                    .ok_or(StorageError::Unavailable)?
                    .open_table(RESOURCE_FENCES)
                    .map_err(|_| StorageError::Unavailable)?;
            for resource in resources {
                if table
                    .get(resource.as_bytes().as_slice())
                    .map_err(|_| StorageError::Unavailable)?
                    .is_some()
                {
                    return Err(StorageError::Unavailable);
                }
            }
            for resource in resources {
                table
                    .insert(resource.as_bytes().as_slice(), value)
                    .map_err(|_| StorageError::Unavailable)?;
            }
        }
        Box::new(tx).commit_durable()
    }

    pub(crate) fn begin_guarded_write(&self) -> Result<GuardedWriteTx<'_>, StorageError> {
        self.check_identity()?;
        let mut tx = self
            .database
            .begin_write()
            .map_err(|_| StorageError::Unavailable)?;
        tx.set_durability(redb::Durability::Immediate);
        Ok(GuardedWriteTx(WriteTx {
            tx: Some(tx),
            staged: Default::default(),
            guard: Some(self),
        }))
    }

    /// Returns all fencing for restoration until resources are provided.
    pub fn fences(&self) -> Result<Vec<carosign_core::ids::ResourceId>, StorageError> {
        self.read_fences(|_| Ok(()))
    }

    pub fn fences_with_operations(&self) -> Result<super::FenceInventory, StorageError> {
        let mut operations = Some(std::collections::BTreeSet::new());
        let resources = self.read_fences(|value| {
            if value == RESOURCE_FENCE_VALUE {
                operations = None;
            } else if let Some(operations) = &mut operations {
                operations.insert(FenceScope::encoded_operation(value).ok_or(
                    StorageError::Corrupt {
                        what: "неверная операция ограждения",
                    },
                )?);
            }
            Ok(())
        })?;
        Ok(super::FenceInventory {
            resources,
            operations,
        })
    }

    pub fn scoped_fences(&self) -> Result<Vec<FenceScope>, StorageError> {
        let mut scopes = Vec::new();
        self.read_fences(|value| {
            if value != RESOURCE_FENCE_VALUE {
                let scope = FenceScope::decode(value).ok_or(StorageError::Corrupt {
                    what: "неверная область ограждения",
                })?;
                if !scopes
                    .iter()
                    .any(|entry: &FenceScope| entry.encode() == value)
                {
                    scopes.push(scope);
                }
            }
            Ok(())
        })?;
        Ok(scopes)
    }

    fn read_fences(
        &self,
        mut visit: impl FnMut(&[u8]) -> Result<(), StorageError>,
    ) -> Result<Vec<carosign_core::ids::ResourceId>, StorageError> {
        self.check_identity()?;
        let tx = self
            .database
            .begin_read()
            .map_err(|_| StorageError::Unavailable)?;
        let table = tx
            .open_table(RESOURCE_FENCES)
            .map_err(|_| StorageError::Unavailable)?;
        let mut result = Vec::new();
        for entry in table.iter().map_err(|_| StorageError::Unavailable)? {
            let (key, value) = entry.map_err(|_| StorageError::Unavailable)?;
            let id: [u8; 16] = key.value().try_into().map_err(|_| StorageError::Corrupt {
                what: "неверное ограждение ресурса",
            })?;
            if id == [0; 16]
                || (value.value() != RESOURCE_FENCE_VALUE
                    && scoped_fence_resources(value.value()).is_none_or(|ids| !ids.contains(&id)))
            {
                return Err(StorageError::Corrupt {
                    what: "неверное ограждение ресурса",
                });
            }
            if value.value() != RESOURCE_FENCE_VALUE {
                for member in
                    scoped_fence_resources(value.value()).ok_or(StorageError::Corrupt {
                        what: "неверный контекст ограждения",
                    })?
                {
                    if table
                        .get(member.as_slice())
                        .map_err(|_| StorageError::Unavailable)?
                        .is_none_or(|other| other.value() != value.value())
                    {
                        return Err(StorageError::Corrupt {
                            what: "неполный набор ограждения",
                        });
                    }
                }
            }
            visit(value.value())?;
            result.push(carosign_core::ids::ResourceId::from_bytes(id).map_err(|_| {
                StorageError::Corrupt {
                    what: "неверное ограждение ресурса",
                }
            })?);
        }
        self.check_identity()?;
        Ok(result)
    }
}
