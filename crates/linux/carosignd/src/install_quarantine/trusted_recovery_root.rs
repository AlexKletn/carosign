use super::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, Path, QuarantineFailure};

/// The fixed absolute path of the systemic root of recovery.
///
/// The verification does not confirm the signature and does not in itself permit the launch.
/// All components must be root-owned and protected from group and other records; links and
/// clicks to parent directories are not allowed.
pub(crate) struct TrustedRecoveryRoot {
    pub(super) path: std::path::PathBuf,
    pub(super) directory: OwnedFd,
}

impl TrustedRecoveryRoot {
    pub(super) fn try_clone(&self) -> Result<Self, QuarantineFailure> {
        Ok(Self {
            path: self.path.clone(),
            directory: self
                .directory
                .try_clone()
                .map_err(|_| QuarantineFailure::Preparation)?,
        })
    }

    pub(crate) fn open(path: &Path) -> Result<Self, QuarantineFailure> {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Component;

        let mut components = path.components();
        if components.next() != Some(Component::RootDir) {
            return Err(QuarantineFailure::Preparation);
        }
        let root = CString::new("/").expect("корень не содержит NUL");
        // SAFETY: root is a string with a final zero; a successful fd becomes a string with a
        // final zero. OwnedFd.
        let raw = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if raw < 0 {
            return Err(QuarantineFailure::Preparation);
        }
        let mut directory = unsafe { OwnedFd::from_raw_fd(raw) };
        Self::check_directory(directory.as_fd())?;
        for component in components {
            let Component::Normal(name) = component else {
                return Err(QuarantineFailure::Preparation);
            };
            let name = CString::new(name.as_bytes()).map_err(|_| QuarantineFailure::Preparation)?;
            // SAFETY: the parent is retained; O_NOFOLLOW excludes clicking on the link.
            let raw = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if raw < 0 {
                return Err(QuarantineFailure::Preparation);
            }
            directory = unsafe { OwnedFd::from_raw_fd(raw) };
            Self::check_directory(directory.as_fd())?;
        }
        Ok(Self {
            path: path.to_owned(),
            directory,
        })
    }

    fn check_directory(fd: BorrowedFd<'_>) -> Result<(), QuarantineFailure> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fd is held by the caller, stat indicates the allowable memory.
        if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
            return Err(QuarantineFailure::Preparation);
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
            || stat.st_uid != 0
            || stat.st_mode & 0o022 != 0
        {
            return Err(QuarantineFailure::Preparation);
        }
        Ok(())
    }

    pub(super) fn revalidate(&self) -> Result<(), QuarantineFailure> {
        let fresh = Self::open(&self.path)?;
        let identity =
            |fd: BorrowedFd<'_>| -> Result<(libc::dev_t, libc::ino_t), QuarantineFailure> {
                let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
                if unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
                    return Err(QuarantineFailure::Preparation);
                }
                let stat = unsafe { stat.assume_init() };
                Ok((stat.st_dev, stat.st_ino))
            };
        if identity(self.directory.as_fd())? != identity(fresh.directory.as_fd())? {
            return Err(QuarantineFailure::Preparation);
        }
        Ok(())
    }
}
