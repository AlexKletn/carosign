use std::fs::{self, File};
use std::io::{self, ErrorKind};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(crate) fn publish_new(
    path: &Path,
    parent: &Path,
    write: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let staging = tempfile::Builder::new()
        .prefix(".carosign-init-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(parent)?;
    let staged = staging.path().join("initial");
    write(&staged)?;
    File::open(&staged)?.sync_all()?;
    match fs::hard_link(&staged, path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    File::open(parent)?.sync_all()
}
