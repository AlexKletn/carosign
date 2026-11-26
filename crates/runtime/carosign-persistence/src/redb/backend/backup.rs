use super::{Database, PreflightBackend, StorageError};

fn backup_entry_path(directory: &std::fs::File, name: &std::ffi::OsStr) -> std::path::PathBuf {
    use std::os::fd::AsRawFd;
    std::path::PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(name)
}

fn backup_identity(file: &std::fs::File, max_size: u64) -> Result<BackupIdentity, StorageError> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{FileExt, MetadataExt};
    let before = file.metadata().map_err(|_| StorageError::Unavailable)?;
    if !before.is_file()
        || before.nlink() == 0
        || before.ino() == 0
        || before.len() == 0
        || before.len() > max_size
    {
        return Err(StorageError::Corrupt {
            what: "недопустимая копия базы",
        });
    }
    let mut hash = Sha256::new();
    let mut offset = 0u64;
    let mut chunk = [0u8; 65536];
    while offset < before.len() {
        let count = usize::try_from((before.len() - offset).min(chunk.len() as u64))
            .map_err(|_| StorageError::Unavailable)?;
        let read = file
            .read_at(&mut chunk[..count], offset)
            .map_err(|_| StorageError::Unavailable)?;
        if read == 0 {
            return Err(StorageError::Corrupt {
                what: "усечённая копия базы",
            });
        }
        hash.update(&chunk[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or(StorageError::Unavailable)?;
    }
    let after = file.metadata().map_err(|_| StorageError::Unavailable)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
        || after.nlink() == 0
    {
        return Err(StorageError::Corrupt {
            what: "копия изменилась при чтении",
        });
    }
    Ok(BackupIdentity {
        device: before.dev(),
        inode: before.ino(),
        length: before.len(),
        digest: hash.finalize().into(),
    })
}

mod verified_v3_backup;
pub(crate) use verified_v3_backup::VerifiedV3Backup;
mod backup_identity;
pub(crate) use backup_identity::BackupIdentity;
mod created_v3_backup;
#[cfg(test)]
pub(crate) use created_v3_backup::CreatedV3Backup;
