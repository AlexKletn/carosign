use super::super::verity;
use super::InputError;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

/// The measurement must be derived from an independent system policy; it does not confirm the
/// bootloader and the library.
pub(in crate::discovery) struct PinnedVerifier {
    pub(super) executable: OwnedFd,
    pub(super) verity: Option<[u8; 32]>,
}

impl PinnedVerifier {
    pub(in crate::discovery) fn prepare(
        executable: BorrowedFd<'_>,
        approved_verity: &[u8; 32],
    ) -> Result<Self, InputError> {
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: The buffer and the descriptor held are valid.
        if unsafe { libc::fstat(executable.as_raw_fd(), &mut stat) } != 0 {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG
            || stat.st_uid != 0
            || stat.st_mode & 0o022 != 0
            || stat.st_mode & 0o111 == 0
        {
            return Err(InputError::InvalidFile);
        }
        if !verity::matches_approved_digest(executable, approved_verity)? {
            return Err(InputError::VerifierUnapproved);
        }
        let duplicate = unsafe { libc::fcntl(executable.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate < 0 {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        Ok(Self {
            executable: unsafe { OwnedFd::from_raw_fd(duplicate) },
            verity: Some(*approved_verity),
        })
    }

    #[cfg(test)]
    pub(in crate::discovery) fn fixture(executable: BorrowedFd<'_>) -> Result<Self, InputError> {
        let duplicate = unsafe { libc::fcntl(executable.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate < 0 {
            return Err(InputError::Io(io::Error::last_os_error()));
        }
        Ok(Self {
            executable: unsafe { OwnedFd::from_raw_fd(duplicate) },
            verity: None,
        })
    }
}
