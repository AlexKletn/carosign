use super::{BackupIdentity, StorageError, VerifiedV3Backup, backup_entry_path, backup_identity};

/// Holds the lock of the original file and the verified copy. It's not permission to migrate:
/// it requires the admission of the old binary and the system fence of transactions for the
/// duration of subsequent transactions.
#[allow(dead_code)]
pub(crate) struct CreatedV3Backup {
    source: std::fs::File,
    _source_lock: redb::backends::FileBackend,
    source_identity: BackupIdentity,
    pub(super) directory: std::fs::File,
    pub(super) name: std::ffi::OsString,
    pub(crate) backup: VerifiedV3Backup,
}

impl CreatedV3Backup {
    /// The directory and the source file are transferred by the already open trusted owner.
    /// The existing file is not overwritten; the remainder is not deleted in case of an
    /// error.
    #[allow(dead_code)]
    pub(crate) fn create(
        source: std::fs::File,
        directory: std::fs::File,
        name: &std::ffi::OsStr,
        max_size: u64,
    ) -> Result<Self, StorageError> {
        use std::io::{Read, Seek, SeekFrom, Write};
        use std::os::unix::fs::OpenOptionsExt;
        use std::path::{Component, Path};

        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || !directory
                .metadata()
                .map_err(|_| StorageError::Unavailable)?
                .is_dir()
        {
            return Err(StorageError::Unavailable);
        }
        let source_lock = redb::backends::FileBackend::new(
            source.try_clone().map_err(|_| StorageError::Unavailable)?,
        )
        .map_err(|_| StorageError::Unavailable)?;
        let before = backup_identity(&source, max_size)?;
        let path = backup_entry_path(&directory, name);
        let mut destination = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| StorageError::Unavailable)?;
        let mut input = source.try_clone().map_err(|_| StorageError::Unavailable)?;
        input
            .seek(SeekFrom::Start(0))
            .map_err(|_| StorageError::Unavailable)?;
        let copied = std::io::copy(&mut input.take(before.length), &mut destination)
            .map_err(|_| StorageError::Unavailable)?;
        destination.flush().map_err(|_| StorageError::Unavailable)?;
        destination
            .sync_all()
            .map_err(|_| StorageError::Unavailable)?;
        directory
            .sync_all()
            .map_err(|_| StorageError::Unavailable)?;
        if copied != before.length || backup_identity(&source, max_size)? != before {
            return Err(StorageError::Corrupt {
                what: "исходная база изменилась при копировании",
            });
        }
        let backup = VerifiedV3Backup::inspect(destination, max_size)?;
        let copy = backup.recheck(max_size)?;
        if copy.length != before.length || copy.digest != before.digest {
            return Err(StorageError::Corrupt {
                what: "копия не совпадает с исходной базой",
            });
        }
        let result = Self {
            source,
            _source_lock: source_lock,
            source_identity: before,
            directory,
            name: name.to_owned(),
            backup,
        };
        result.recheck(max_size)?;
        Ok(result)
    }

    #[allow(dead_code)]
    pub(crate) fn recheck(&self, max_size: u64) -> Result<BackupIdentity, StorageError> {
        use std::os::unix::fs::MetadataExt;
        if backup_identity(&self.source, max_size)? != self.source_identity {
            return Err(StorageError::Corrupt {
                what: "исходная база изменилась",
            });
        }
        let identity = self.backup.recheck(max_size)?;
        let entry = std::fs::symlink_metadata(backup_entry_path(&self.directory, &self.name))
            .map_err(|_| StorageError::Unavailable)?;
        if !entry.is_file() || entry.dev() != identity.device || entry.ino() != identity.inode {
            return Err(StorageError::Corrupt {
                what: "имя копии заменено",
            });
        }
        Ok(identity)
    }
}
