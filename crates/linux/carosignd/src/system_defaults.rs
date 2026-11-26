use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use carosignd::config::{Config, DEFAULT_POLICY, DEFAULT_PROVIDER_ALLOWLIST};
use carosignd::installation::trusted_store_path;

use crate::initial_file::publish_new;

pub(crate) fn prepare(config: &Config) -> io::Result<()> {
    // SAFETY: geteuid takes no arguments and has no memory safety preconditions.
    if unsafe { libc::geteuid() } != 0 {
        return Ok(());
    }
    if config.policy == DEFAULT_POLICY {
        create_default(DEFAULT_POLICY, b"uid=0 subject=0 actions=inventory\n")?;
    }
    if config.provider_profile.is_none() {
        create_default(
            DEFAULT_PROVIDER_ALLOWLIST,
            b"# name uid gid authenticator profile\n",
        )?;
    }
    Ok(())
}

fn create_default(path: &str, contents: &[u8]) -> io::Result<()> {
    let path = Path::new(path);
    match fs::symlink_metadata(path) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = path.parent().ok_or(ErrorKind::InvalidInput)?;
    trusted_store_path(parent).map_err(|_| ErrorKind::PermissionDenied)?;
    publish_new(path, parent, |staged| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(staged)?
            .write_all(contents)
    })
}
