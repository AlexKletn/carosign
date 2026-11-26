use redb::{ReadableTableMetadata, TableHandle};

use super::{
    AUTHENTICATORS, Database, PROVIDER_STATE, Path, PreflightBackend, ReadTxRef, StorageError,
    TEMPLATE_BLOBS,
};

/// Isolated reading of quarantine under exclusive file lock.
///
/// The opening of redb and any service records of it are sent to memory.Type does not provide
/// any transaction record, nor access to the working repository, and does not confirm the
/// origin of the installation and the right to quarantine.
pub struct QuarantineRecoveryStore {
    database: Database,
    package_key: Vec<u8>,
    installation_key: Vec<u8>,
}

impl QuarantineRecoveryStore {
    /// Checks the exact quarantine installation and holds the redb lock.
    /// The initial base does not change even if it is rejected or successfully opened.
    pub fn open(
        path: &Path,
        package_key: &[u8],
        installation_key: &[u8],
    ) -> Result<Self, StorageError> {
        use std::os::unix::fs::OpenOptionsExt;
        if !path.is_absolute() || package_key.is_empty() || installation_key.is_empty() {
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
        let base = std::sync::Arc::new(
            redb::backends::FileBackend::new(file).map_err(|_| StorageError::Unavailable)?,
        );
        let overlay = PreflightBackend::new(base).map_err(|_| StorageError::Unavailable)?;
        let database = Database::builder()
            .create_with_backend(overlay)
            .map_err(|_| StorageError::Corrupt {
                what: "база не открывается для проверки",
            })?;
        {
            let tx = database
                .begin_read()
                .map_err(|_| StorageError::Unavailable)?;
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
            crate::installation_records::read_quarantined_snapshot(
                &tx,
                package_key,
                installation_key,
            )
            .map_err(|_| StorageError::Corrupt {
                what: "карантинная установка не прошла проверку",
            })?;
            let snapshot = ReadTxRef { tx: &tx };
            crate::mutation_store::scan_prepared_v4_snapshot(&snapshot)?;
        }
        Ok(Self {
            database,
            package_key: package_key.to_vec(),
            installation_key: installation_key.to_vec(),
        })
    }

    /// Returns the unverified contents of the quarantine from the system source.
    pub fn snapshot(
        &self,
    ) -> Result<
        crate::installation_records::QuarantinedSnapshot,
        crate::installation_records::SnapshotRefusal,
    > {
        let tx = self
            .database
            .begin_read()
            .map_err(|_| crate::installation_records::SnapshotRefusal::Corrupt)?;
        crate::installation_records::read_quarantined_snapshot(
            &tx,
            &self.package_key,
            &self.installation_key,
        )
    }

    /// Verifies both records and the marker again, without allowing start or write.
    pub fn revalidate(
        &self,
        snapshot: &crate::installation_records::QuarantinedSnapshot,
    ) -> Result<(), crate::installation_records::SnapshotRefusal> {
        crate::installation_records::revalidate_quarantined_snapshot(
            &self.database,
            &self.package_key,
            &self.installation_key,
            snapshot,
        )
    }
}
