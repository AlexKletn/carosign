use super::{BackupIdentity, Database, PreflightBackend, StorageError, backup_identity};

/// Holds the verified v3 fileshot and its redb lock.
/// This is not an indication of equality of the baseline and non-admission to migration.
#[allow(dead_code)]
pub(crate) struct VerifiedV3Backup {
    pub(super) file: std::fs::File,
    _lock: std::sync::Arc<redb::backends::FileBackend>,
    identity: BackupIdentity,
}

impl VerifiedV3Backup {
    /// It checks a retained file without writing to it, and the migration composition must
    /// separately prove the source of the copy, the fsync, the external lock, and the old
    /// binary; this object does not create any of these evidences.
    #[allow(dead_code)]
    pub(crate) fn inspect(file: std::fs::File, max_size: u64) -> Result<Self, StorageError> {
        let before = backup_identity(&file, max_size)?;
        let backend_file = file.try_clone().map_err(|_| StorageError::Unavailable)?;
        let base = std::sync::Arc::new(
            redb::backends::FileBackend::new(backend_file)
                .map_err(|_| StorageError::Unavailable)?,
        );
        let overlay = PreflightBackend::new(std::sync::Arc::clone(&base))
            .map_err(|_| StorageError::Unavailable)?;
        {
            let probe = Database::builder()
                .create_with_backend(overlay)
                .map_err(|_| StorageError::Corrupt {
                    what: "копия базы не открывается",
                })?;
            let snapshot = probe.begin_read().map_err(|_| StorageError::Unavailable)?;
            crate::migration::preflight_v3_snapshot(snapshot).map_err(|_| {
                StorageError::Corrupt {
                    what: "копия v3 не прошла проверку",
                }
            })?;
        }
        if backup_identity(&file, max_size)? != before {
            return Err(StorageError::Corrupt {
                what: "копия изменилась во время проверки",
            });
        }
        Ok(Self {
            file,
            _lock: base,
            identity: before,
        })
    }

    /// Reconfirms bytes and identity of the retained file.
    #[allow(dead_code)]
    pub(crate) fn recheck(&self, max_size: u64) -> Result<BackupIdentity, StorageError> {
        let current = backup_identity(&self.file, max_size)?;
        if current != self.identity {
            return Err(StorageError::Corrupt {
                what: "проверенная копия изменилась",
            });
        }
        Ok(current)
    }
}
