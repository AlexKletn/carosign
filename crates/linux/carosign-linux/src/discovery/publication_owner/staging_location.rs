use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use super::{directory_metadata::directory_metadata, duplicate_fd::duplicate_fd};

/// Retains the root of the installation and staging; the name is not an authentication of the
/// object and is re-authorized before each use.
pub(super) struct StagingLocation {
    installation_root: OwnedFd,
    pub(super) staging: OwnedFd,
    pub(super) version_dir: String,
    pub(super) owner: u32,
}

impl StagingLocation {
    /// Creation does not confirm trust in the owner: the system owner is obliged to transfer
    /// the retained, independently verified root of the installation.
    pub(super) fn bind(
        installation_root: BorrowedFd<'_>,
        staging: BorrowedFd<'_>,
        version_dir: &str,
        owner: u32,
    ) -> io::Result<Self> {
        let location = Self {
            installation_root: duplicate_fd(installation_root)?,
            staging: duplicate_fd(staging)?,
            version_dir: version_dir.to_owned(),
            owner,
        };
        location.revalidate()?;
        Ok(location)
    }

    /// Verifies open staging with the path from the retained root. This is a snapshot:
    /// rechecking does not serialize the subsequent publication with the replacement path.
    pub(super) fn revalidate(&self) -> io::Result<()> {
        let path = &self.version_dir;
        if path.is_empty()
            || path.len() > 1024
            || path.split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.len() > 255
                    || part
                        .bytes()
                        .any(|byte| byte == 0 || byte.is_ascii_control())
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe version path",
            ));
        }
        directory_metadata(self.installation_root.as_fd(), self.owner)?;
        let expected = directory_metadata(self.staging.as_fd(), self.owner)?;
        let mut current = duplicate_fd(self.installation_root.as_fd())?;
        for part in path.split('/') {
            let part = std::ffi::CString::new(part)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "unsafe component"))?;
            // SAFETY: name - one verified component, the descriptor is held.
            let raw = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    part.as_ptr(),
                    libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            current = unsafe { OwnedFd::from_raw_fd(raw) };
            directory_metadata(current.as_fd(), self.owner)?;
        }
        let actual = directory_metadata(current.as_fd(), self.owner)?;
        if actual.st_dev != expected.st_dev || actual.st_ino != expected.st_ino {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "staging replaced",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "staging_location.tests.rs"]
mod tests;
