use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use carosign_persistence::redb::backend::RedbStore;
use carosignd::installation::trusted_store_path;

use crate::initial_file::publish_new;

pub(crate) fn prepare(path: &Path) -> io::Result<()> {
    // SAFETY: geteuid takes no arguments and has no memory safety preconditions.
    let privileged = unsafe { libc::geteuid() } == 0;
    let parent = path
        .parent()
        .filter(|parent| parent.is_absolute())
        .ok_or_else(untrusted_path)?;
    if privileged && trusted_store_path(parent).is_err() {
        return Err(untrusted_path());
    }

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            publish_new(path, parent, |staged| {
                RedbStore::create_current(staged)
                    .map(drop)
                    .map_err(|error| {
                        io::Error::other(format!("не удалось создать хранилище: {error}"))
                    })
            })?;
            fs::symlink_metadata(path)?
        }
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || (privileged && (metadata.uid() != 0 || metadata.mode() & 0o077 != 0))
    {
        return Err(untrusted_path());
    }
    Ok(())
}

fn untrusted_path() -> io::Error {
    io::Error::new(ErrorKind::PermissionDenied, "путь базы не защищён")
}
