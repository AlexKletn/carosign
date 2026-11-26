use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use carosign_linux::sys;

pub(super) struct WorkerListener {
    pub(super) fd: OwnedFd,
    pub(super) path: PathBuf,
}

impl WorkerListener {
    pub(super) fn bind(directory: &Path, id: &[u8; 16]) -> io::Result<Self> {
        use std::fmt::Write;
        let root = Path::new("/run");
        if !directory.starts_with(root)
            || directory
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        for ancestor in directory
            .ancestors()
            .take_while(|path| path.starts_with(root))
        {
            let metadata = std::fs::symlink_metadata(ancestor)?;
            if !ancestor.is_absolute()
                || !metadata.is_dir()
                || metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
            {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
        }
        let mut name = String::from("worker-");
        for byte in id {
            let _ = write!(name, "{byte:02x}");
        }
        let path = directory.join(name);
        let fd = sys::listen_unix(path.to_str().ok_or(io::ErrorKind::InvalidInput)?)
            .map_err(|error| io::Error::from_raw_os_error(error.errno))?;
        let listener = Self { fd, path };
        std::fs::set_permissions(&listener.path, std::fs::Permissions::from_mode(0o622))?;
        Ok(listener)
    }
}

impl Drop for WorkerListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
