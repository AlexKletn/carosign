use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Component, Path, PathBuf};

use carosign_core::ids::AuthenticatorId;

use super::published_device::PublishedDevice;

pub(super) struct Publication {
    pub(super) name: String,
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) devices: BTreeMap<AuthenticatorId, PublishedDevice>,
}

impl Publication {
    pub(super) fn endpoint(&self, root: &Path) -> io::Result<PathBuf> {
        for directory in root.ancestors() {
            if directory
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            let metadata = fs::symlink_metadata(directory)?;
            if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
        }
        let directory = root.join(&self.name);
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.uid() != self.uid
            || metadata.gid() != self.gid
            || metadata.mode() & 0o022 != 0
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let endpoint = directory.join("control.sock");
        let metadata = fs::symlink_metadata(&endpoint)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != self.uid
            || metadata.gid() != self.gid
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(endpoint)
    }
}
