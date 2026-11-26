use super::super::verity;
use super::Entry;
use super::RuntimeError;
use super::RuntimeFile;
use crate::process::runtime_mount;
use std::collections::BTreeSet;
use std::io;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

const FILE_LIMIT: usize = 128;

/// Holds measured files until a closed root is built.
pub(in crate::discovery) struct PinnedRuntime {
    pub(super) entries: Vec<Entry>,
}

pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 4096
        && !name.starts_with('/')
        && name.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 255
                && !part.as_bytes().contains(&0)
        })
}

impl PinnedRuntime {
    /// Copies retained immutable files into private tmpfs.
    /// To fix the source and confirm the fs-verity must prepare runtime.
    /// The caller is obliged to close the record in tmpfs before the checker starts.
    pub(in crate::discovery) fn copy_into(&self, root: BorrowedFd<'_>) -> Result<(), RuntimeError> {
        use std::io::Write;
        use std::os::unix::fs::FileExt;
        for entry in &self.entries {
            let mut parent: Option<OwnedFd> = None;
            let parts: Vec<&str> = entry.name.split('/').collect();
            for part in &parts[..parts.len() - 1] {
                let part = std::ffi::CString::new(*part).map_err(|_| RuntimeError::InvalidName)?;
                let at = parent.as_ref().map_or(root.as_raw_fd(), AsRawFd::as_raw_fd);
                if unsafe { libc::mkdirat(at, part.as_ptr(), 0o700) } != 0 {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::EEXIST) {
                        return Err(RuntimeError::Io(error));
                    }
                }
                let fd = unsafe {
                    libc::openat(
                        at,
                        part.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(RuntimeError::Io(io::Error::last_os_error()));
                }
                parent = Some(unsafe { OwnedFd::from_raw_fd(fd) });
            }
            let at = parent.as_ref().map_or(root.as_raw_fd(), AsRawFd::as_raw_fd);
            let name = std::ffi::CString::new(*parts.last().ok_or(RuntimeError::InvalidName)?)
                .map_err(|_| RuntimeError::InvalidName)?;
            let fd = unsafe {
                libc::openat(
                    at,
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o700,
                )
            };
            if fd < 0 {
                return Err(RuntimeError::Io(io::Error::last_os_error()));
            }
            let mut output = unsafe { std::fs::File::from_raw_fd(fd) };
            let dup = unsafe { libc::fcntl(entry.source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
            if dup < 0 {
                return Err(RuntimeError::Io(io::Error::last_os_error()));
            }
            let source = unsafe { std::fs::File::from_raw_fd(dup) };
            let mut offset = 0u64;
            let mut buf = [0u8; 16384];
            loop {
                let n = source.read_at(&mut buf, offset).map_err(RuntimeError::Io)?;
                if n == 0 {
                    break;
                }
                offset = offset
                    .checked_add(n as u64)
                    .ok_or(RuntimeError::InvalidFile)?;
                if offset > 256 * 1024 * 1024 {
                    return Err(RuntimeError::InvalidFile);
                }
                output.write_all(&buf[..n]).map_err(RuntimeError::Io)?;
            }
            if unsafe { libc::fchmod(output.as_raw_fd(), 0o500) } != 0 {
                return Err(RuntimeError::Io(io::Error::last_os_error()));
            }
        }
        Ok(())
    }
    /// Verifies the executable file in a closed root with the retained verifier.
    /// The coincidence itself does not confirm the origin of policy and dependency.
    pub(in crate::discovery) fn contains_verifier(
        &self,
        name: &str,
        verifier: BorrowedFd<'_>,
    ) -> io::Result<bool> {
        let Some(entry) = self.entries.iter().find(|entry| entry.name == name) else {
            return Ok(false);
        };
        let stat = |fd: BorrowedFd<'_>| -> io::Result<libc::stat> {
            // SAFETY: The core fills the buffer for the retained descriptor.
            let mut value: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(fd.as_raw_fd(), &mut value) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(value)
        };
        let mounted = stat(entry.source.as_fd())?;
        let checked = stat(verifier)?;
        Ok(mounted.st_dev == checked.st_dev
            && mounted.st_ino == checked.st_ino
            && mounted.st_mode & libc::S_IFMT == libc::S_IFREG
            && checked.st_mode & libc::S_IFMT == libc::S_IFREG)
    }

    /// Accepts only regular root-owned files with a policy dimension.
    pub(in crate::discovery) fn prepare(files: &[RuntimeFile<'_>]) -> Result<Self, RuntimeError> {
        if files.is_empty() || files.len() > FILE_LIMIT {
            return Err(RuntimeError::InvalidFile);
        }
        let mut names = BTreeSet::new();
        let mut entries = Vec::with_capacity(files.len());
        for file in files {
            if !valid_name(file.name) {
                return Err(RuntimeError::InvalidName);
            }
            if !names.insert(file.name) {
                return Err(RuntimeError::DuplicateName);
            }
            // SAFETY: The core fills the stat of the retained source.
            let mut stat: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(file.source.as_raw_fd(), &mut stat) } != 0 {
                return Err(RuntimeError::Io(io::Error::last_os_error()));
            }
            if stat.st_mode & libc::S_IFMT != libc::S_IFREG
                || stat.st_uid != 0
                || stat.st_mode & 0o022 != 0
            {
                return Err(RuntimeError::InvalidFile);
            }
            if !verity::matches_approved_digest(file.source, &file.approved_verity)
                .map_err(RuntimeError::Io)?
            {
                return Err(RuntimeError::Unapproved);
            }
            // SAFETY: fcntl returns a new descriptor of the retained file.
            let raw = unsafe { libc::fcntl(file.source.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
            if raw < 0 {
                return Err(RuntimeError::Io(io::Error::last_os_error()));
            }
            entries.push(Entry {
                name: file.name.to_owned(),
                source: unsafe { OwnedFd::from_raw_fd(raw) },
            });
        }
        Ok(Self { entries })
    }

    /// Attached read-only files to an unpublished root.
    ///
    /// The caller must own a separate mount namespace and an empty root that is closed to
    /// outsiders, and if any mistake is made, the entire root is discarded.
    /// Going to the root and confirming the completeness of the files are performed
    /// separately.
    pub(in crate::discovery) fn attach(&self, root: BorrowedFd<'_>) -> Result<(), RuntimeError> {
        for entry in &self.entries {
            runtime_mount::attach_file(root, &entry.name, entry.source.as_fd())
                .map_err(RuntimeError::Mount)?;
        }
        Ok(())
    }
}
