use super::{
    AUTHENTICATORS, Database, ENROLLMENT_INDEX, ENROLLMENTS, IDEMPOTENCY, INSTALLATIONS, META,
    MUTATIONS, MetadataStore, MultimapTableHandle, OUTBOX, PACKAGES, PROVIDER_STATE, Path,
    PreflightBackend, ReadTransaction, ReadTx, ReadTxRef, ReadableTable, ReadableTableMetadata,
    SCHEMA_KEY, SCHEMA_V3, SCHEMA_V4, SCOPE_LOCK, SharedBackend, StorageError, TEMPLATE_BLOBS,
    Table, TableHandle, WriteTransaction, WriteTx, definition, fmt,
};

/// Storage on redb.
pub struct RedbStore {
    database: Database,
}

impl fmt::Debug for RedbStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RedbStore(..)")
    }
}

impl RedbStore {
    /// Reads a quarantine shot without permission to launch or quarantine.
    pub fn quarantined_installation_snapshot(
        &self,
        package_key: &[u8],
        installation_key: &[u8],
    ) -> Result<
        crate::installation_records::QuarantinedSnapshot,
        crate::installation_records::SnapshotRefusal,
    > {
        let tx = self
            .database
            .begin_read()
            .map_err(|_| crate::installation_records::SnapshotRefusal::Corrupt)?;
        crate::installation_records::read_quarantined_snapshot(&tx, package_key, installation_key)
    }

    /// Reconfirms the entire quarantine image in the new transaction.
    pub fn revalidate_quarantined_installation(
        &self,
        package_key: &[u8],
        installation_key: &[u8],
        snapshot: &crate::installation_records::QuarantinedSnapshot,
    ) -> Result<(), crate::installation_records::SnapshotRefusal> {
        crate::installation_records::revalidate_quarantined_snapshot(
            &self.database,
            package_key,
            installation_key,
            snapshot,
        )
    }

    /// Records measurements of the signed archive and signature along with quarantine.
    /// Measurements do not confirm origin and do not permit launch.
    pub fn quarantine_initial_installation_with_provenance(
        &self,
        package: &crate::installation_records::PackageRecord,
        installation: &crate::installation_records::ProviderInstallationRecord,
        provenance: [[u8; 32]; 2],
        check: impl FnMut() -> bool,
    ) -> Result<(), crate::installation_records::PublicationFailure> {
        let mut key = installation.package_id.clone();
        key.extend_from_slice(&installation.profile_id);
        crate::installation_records::publish_initial_with_provenance(
            &self.database,
            package,
            &key,
            installation,
            Some(provenance),
            check,
        )
    }
    /// Records the installation exclusively with a stable quarantine barrier.
    /// Successful fixation returns RecoveryRequired, not launch authorization.
    pub fn quarantine_initial_installation(
        &self,
        package: &crate::installation_records::PackageRecord,
        installation: &crate::installation_records::ProviderInstallationRecord,
        check: impl FnMut() -> bool,
    ) -> Result<(), crate::installation_records::PublicationFailure> {
        let mut key = installation.package_id.clone();
        key.extend_from_slice(&installation.profile_id);
        crate::installation_records::publish_initial_with_policy_check(
            &self.database,
            package,
            &key,
            installation,
            check,
        )
    }
    /// Opens v4 with checked packages, installations, registry and supported changes. The
    /// rest of the records are waiting to check the links.
    pub fn open_current_guarded(path: &Path) -> Result<Self, StorageError> {
        Self::open_schema_guarded(path, SCHEMA_V4)
    }

    fn open_schema_guarded(path: &Path, expected_schema: &[u8]) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| StorageError::Unavailable)?;
        Self::open_schema_guarded_file(file, expected_schema)
    }

    fn open_schema_guarded_file(
        file: std::fs::File,
        expected_schema: &[u8],
    ) -> Result<Self, StorageError> {
        let metadata = file.metadata().map_err(|_| StorageError::Unavailable)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(StorageError::Corrupt {
                what: "пустая база",
            });
        }
        let base = std::sync::Arc::new(
            redb::backends::FileBackend::new(file).map_err(|_| StorageError::Unavailable)?,
        );
        let overlay = PreflightBackend::new(std::sync::Arc::clone(&base))
            .map_err(|_| StorageError::Unavailable)?;
        {
            let probe = Database::builder()
                .create_with_backend(overlay)
                .map_err(|_| StorageError::Corrupt {
                    what: "база не открывается для проверки",
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
            ];
            let tables: Vec<_> = tx
                .list_tables()
                .map_err(|_| StorageError::Unavailable)?
                .collect();
            if tables.len() != names.len()
                || tables.iter().any(|table| !names.contains(&table.name()))
                || tx
                    .list_multimap_tables()
                    .map_err(|_| StorageError::Unavailable)?
                    .next()
                    .is_some()
            {
                return Err(StorageError::Corrupt {
                    what: "неизвестный состав таблиц v4",
                });
            }
            let meta = tx.open_table(META).map_err(|_| StorageError::Corrupt {
                what: "таблица meta не открывается",
            })?;
            if meta.len().map_err(|_| StorageError::Unavailable)? != 1
                || meta
                    .get(SCHEMA_KEY)
                    .map_err(|_| StorageError::Unavailable)?
                    .is_none_or(|value| value.value() != expected_schema)
            {
                return Err(StorageError::Corrupt {
                    what: "неизвестная схема или метадата v4",
                });
            }
            for definition in [AUTHENTICATORS, PROVIDER_STATE, TEMPLATE_BLOBS] {
                let table = tx
                    .open_table(definition)
                    .map_err(|_| StorageError::Corrupt {
                        what: "таблица v4 не открывается",
                    })?;
                if !table.is_empty().map_err(|_| StorageError::Unavailable)? {
                    return Err(StorageError::Corrupt {
                        what: "проверка непустой базы v4 ещё не подключена",
                    });
                }
            }
            crate::installation_records::validate_v4_snapshot(&tx).map_err(|_| {
                StorageError::Corrupt {
                    what: "несогласованные пакеты или установки v4",
                }
            })?;
            let snapshot = ReadTxRef { tx: &tx };
            crate::mutation_store::scan_prepared_v4_snapshot(&snapshot)?;
        }
        let database = Database::builder()
            .create_with_backend(SharedBackend(base))
            .map_err(|_| StorageError::Corrupt {
                what: "база не открывается",
            })?;
        Ok(Self { database })
    }

    /// Creates a new V4 database with a single transaction without overwriting the existing
    /// path.
    pub fn create_current(path: &Path) -> Result<Self, StorageError> {
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
        let database = Database::builder()
            .create_file(file)
            .map_err(|_| StorageError::Unavailable)?;
        let store = Self { database };
        let tx = store
            .database
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
            ] {
                tx.open_table(definition)
                    .map_err(|_| StorageError::Unavailable)?;
            }
            tx.open_table(META)
                .map_err(|_| StorageError::Unavailable)?
                .insert(SCHEMA_KEY, SCHEMA_V4)
                .map_err(|_| StorageError::Unavailable)?;
        }
        tx.commit().map_err(|_| StorageError::Unavailable)?;
        std::fs::File::open(path.parent().ok_or(StorageError::Unavailable)?)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| StorageError::Unavailable)?;
        Ok(store)
    }
    /// Checks the marker on an isolated snapshot of redb records, holding it to file lock
    /// until a subsequent present opening, the check does not write to the source base for
    /// unknown pattern or migration.
    pub fn open_v3_guarded(path: &Path) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
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
        // FileBackend It holds the same flock redb until the end of both discoveries.
        let base = std::sync::Arc::new(
            redb::backends::FileBackend::new(file).map_err(|_| StorageError::Unavailable)?,
        );
        let overlay = PreflightBackend::new(std::sync::Arc::clone(&base))
            .map_err(|_| StorageError::Unavailable)?;
        {
            let probe = Database::builder()
                .create_with_backend(overlay)
                .map_err(|_| StorageError::Corrupt {
                    what: "база не открывается для проверки",
                })?;
            let tx = probe.begin_read().map_err(|_| StorageError::Unavailable)?;
            // The installation record belongs to v4. Under the v3 marker, its presence means
            // a partial/unknown diagram, not an empty directory.
            // Check the name without trying to open the table with the intended type: the
            // incompatible type also does not allow launch.
            let tables = tx.list_tables().map_err(|_| StorageError::Corrupt {
                what: "не удалось проверить таблицы схемы",
            })?;
            if tables
                .into_iter()
                .any(|table| table.name() == "provider_installations")
            {
                return Err(StorageError::Corrupt {
                    what: "таблица установок недопустима при схеме v3",
                });
            }
            let multimaps = tx
                .list_multimap_tables()
                .map_err(|_| StorageError::Corrupt {
                    what: "не удалось проверить таблицы схемы",
                })?;
            if multimaps
                .into_iter()
                .any(|table| table.name() == "provider_installations")
            {
                return Err(StorageError::Corrupt {
                    what: "таблица установок недопустима при схеме v3",
                });
            }
            let table = tx.open_table(META).map_err(|_| StorageError::Corrupt {
                what: "таблица meta не открывается",
            })?;
            let marker = table
                .get(SCHEMA_KEY)
                .map_err(|_| StorageError::Unavailable)?;
            if marker.is_none_or(|value| value.value() != SCHEMA_V3) {
                return Err(StorageError::Corrupt {
                    what: "неизвестная схема или незавершённая миграция",
                });
            }
            // Until a separate migration record contract is implemented, any other metadate
            // may contain its incomplete state.
            // The image does not allow you to open the original base for recording.
            for entry in table.iter().map_err(|_| StorageError::Unavailable)? {
                let (key, _) = entry.map_err(|_| StorageError::Unavailable)?;
                if key.value() != SCHEMA_KEY {
                    return Err(StorageError::Corrupt {
                        what: "неизвестная метадата или незавершённая миграция",
                    });
                }
            }
        }
        let database = Database::builder()
            .create_with_backend(SharedBackend(base))
            .map_err(|_| StorageError::Corrupt {
                what: "база не открывается",
            })?;
        let store = Self { database };
        store.validate_tables()?;
        Ok(store)
    }
    /// Opens an existing vault.
    ///
    /// ⚠️ A damaged database is rebuilt: “empty storage” instead of a damaged one is the loss
    /// of all registrations, issued as a blank slate.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let database = Database::open(path).map_err(|_| StorageError::Corrupt {
            what: "база не открывается",
        })?;
        let store = Self { database };
        store.validate_tables()?;
        Ok(store)
    }

    /// Creates unmarked storage for checking the previous format.
    /// For the primary installation, []`Self::create_current`].
    pub fn create(path: &Path) -> Result<Self, StorageError> {
        // create_new Atomically rejects an existing file: redb::create would otherwise open
        // up the existing base, and ensure_tables I'd change her.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| StorageError::Unavailable)?;
        let database = Database::builder()
            .create_file(file)
            .map_err(|_| StorageError::Unavailable)?;
        let store = Self { database };
        store.ensure_tables(false)?;
        Ok(store)
    }

    /// Clearly creates a new base v3; the existing path never opens.
    pub fn create_v3(path: &Path) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
        if !path.is_absolute() {
            return Err(StorageError::Unavailable);
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|_| StorageError::Unavailable)?;
        let database = Database::builder()
            .create_file(file)
            .map_err(|_| StorageError::Unavailable)?;
        let store = Self { database };
        store.ensure_tables(true)?;
        // Fixing redb does not confirm the file name in the directory.
        std::fs::File::open(path.parent().ok_or(StorageError::Unavailable)?)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| StorageError::Unavailable)?;
        Ok(store)
    }

    /// Only the exact v3 marker with no incomplete migration confirms.
    pub fn has_schema_v3(&self) -> Result<bool, StorageError> {
        let tx = self
            .database
            .begin_read()
            .map_err(|_| StorageError::Unavailable)?;
        let table = tx.open_table(META).map_err(|_| StorageError::Corrupt {
            what: "таблица meta не открывается",
        })?;
        let marker = table
            .get(SCHEMA_KEY)
            .map_err(|_| StorageError::Unavailable)?;
        Ok(marker.is_some_and(|value| value.value() == SCHEMA_V3))
    }

    /// Creates all the tables so that the reading of the empty base does not fall.
    ///
    /// ⚠️ `redb` It doesn't create a table when you read; the absence of a table is not "no
    /// records," it's "no diagram."
    fn ensure_tables(&self, initialize_v3: bool) -> Result<(), StorageError> {
        let tx = self
            .database
            .begin_write()
            .map_err(|_| StorageError::Unavailable)?;
        {
            for table in [
                Table::Meta,
                Table::Authenticators,
                Table::Enrollments,
                Table::EnrollmentIndex,
                Table::Mutations,
                Table::MutationScopeLock,
                Table::EffectOutbox,
                Table::Idempotency,
                Table::ProviderState,
                Table::Packages,
                Table::TemplateBlobs,
            ] {
                let _ = tx
                    .open_table(definition(table))
                    .map_err(|_| StorageError::Corrupt {
                        what: "таблица не открывается",
                    })?;
            }
            if initialize_v3 {
                let mut meta = tx.open_table(META).map_err(|_| StorageError::Unavailable)?;
                meta.insert(SCHEMA_KEY, SCHEMA_V3)
                    .map_err(|_| StorageError::Unavailable)?;
            }
        }
        tx.commit().map_err(|_| StorageError::Unavailable)?;
        Ok(())
    }

    /// When you open an existing database, the absence of a table is a failure of the scheme.
    /// Opening should not create tables or migrate.
    fn validate_tables(&self) -> Result<(), StorageError> {
        let tx = self
            .database
            .begin_read()
            .map_err(|_| StorageError::Unavailable)?;
        for table in [
            Table::Meta,
            Table::Authenticators,
            Table::Enrollments,
            Table::EnrollmentIndex,
            Table::Mutations,
            Table::MutationScopeLock,
            Table::EffectOutbox,
            Table::Idempotency,
            Table::ProviderState,
            Table::Packages,
            Table::TemplateBlobs,
        ] {
            let _ = tx
                .open_table(definition(table))
                .map_err(|_| StorageError::Corrupt {
                    what: "отсутствует или не открывается таблица схемы",
                })?;
        }
        Ok(())
    }
}

impl MetadataStore for RedbStore {
    fn begin_read(&self) -> Result<Box<dyn ReadTransaction + '_>, StorageError> {
        let tx = self
            .database
            .begin_read()
            .map_err(|_| StorageError::Unavailable)?;
        Ok(Box::new(ReadTx { tx }))
    }

    fn begin_write(&self) -> Result<Box<dyn WriteTransaction + '_>, StorageError> {
        let tx = self
            .database
            .begin_write()
            .map_err(|_| StorageError::Unavailable)?;
        Ok(Box::new(WriteTx {
            tx: Some(tx),
            staged: std::collections::BTreeMap::new(),
            guard: None,
        }))
    }
}
