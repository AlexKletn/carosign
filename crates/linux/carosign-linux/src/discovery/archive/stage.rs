use super::ArchiveError;
use super::CheckedArchive;
use super::safe_path;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

fn directory_empty(root: BorrowedFd<'_>) -> Result<bool, ArchiveError> {
    // We open our own description, so as not to change the position of the directory of the
    // caller.
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c".".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let dir = unsafe { libc::fdopendir(fd) };
    if dir.is_null() {
        unsafe { libc::close(fd) };
        return Err(io::Error::last_os_error().into());
    }
    let result = loop {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            break if error.raw_os_error() == Some(0) {
                Ok(true)
            } else {
                Err(ArchiveError::Io(error))
            };
        }
        let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
        if name != c"." && name != c".." {
            break Ok(false);
        }
    };
    unsafe {
        libc::closedir(dir);
    }
    result
}

pub(super) fn same_staged_file(written: &std::fs::Metadata, reopened: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    written.dev() == reopened.dev()
        && written.ino() == reopened.ino()
        && written.len() == reopened.len()
}

/// Extracted files are not yet allowed to be launched or published.
/// The caller is obliged to transfer a new closed installer catalog and destroy it completely
/// in case of any mistake; it is impossible to repeat the extraction into it.
pub(in crate::discovery) fn stage(
    archive: &CheckedArchive,
    root: BorrowedFd<'_>,
) -> Result<BTreeMap<String, OwnedFd>, ArchiveError> {
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: The kernel writes the stat into a dedicated buffer.
    if unsafe { libc::fstat(root.as_raw_fd(), &mut stat) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o077 != 0
    {
        return Err(ArchiveError::Unsafe);
    }
    if !directory_empty(root)? {
        return Err(ArchiveError::Unsafe);
    }
    let mut dirs = BTreeSet::new();
    let mut files = BTreeMap::new();
    for (path, bytes) in &archive.files {
        // Check again: the type should not turn an arbitrary path into a trusted one.
        safe_path(path.as_bytes(), false)?;
        let mut components = path.split('/').peekable();
        let mut parent: Option<OwnedFd> = None;
        let mut prefix = String::new();
        while let Some(component) = components.next() {
            let name = std::ffi::CString::new(component).map_err(|_| ArchiveError::Unsafe)?;
            let parent_fd = parent.as_ref().map_or(root.as_raw_fd(), AsRawFd::as_raw_fd);
            if components.peek().is_some() {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(component);
                if dirs.insert(prefix.clone()) {
                    // SAFETY: parent_fd The name is one verified component.
                    if unsafe { libc::mkdirat(parent_fd, name.as_ptr(), 0o700) } < 0 {
                        return Err(io::Error::last_os_error().into());
                    }
                    // The content has not yet been published, but the directory name must
                    // survive the failure before the installation record becomes available.
                    if unsafe { libc::fsync(parent_fd) } < 0 {
                        return Err(io::Error::last_os_error().into());
                    }
                }
                // SAFETY: We do not follow links and keep an open directory.
                let fd = unsafe {
                    libc::openat(
                        parent_fd,
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error().into());
                }
                parent = Some(unsafe { OwnedFd::from_raw_fd(fd) });
            } else {
                // SAFETY: O_EXCL It prohibits the replacement of an existing facility.
                let fd = unsafe {
                    libc::openat(
                        parent_fd,
                        name.as_ptr(),
                        libc::O_WRONLY
                            | libc::O_CREAT
                            | libc::O_EXCL
                            | libc::O_NOFOLLOW
                            | libc::O_CLOEXEC,
                        0o600,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error().into());
                }
                let mut output = unsafe { std::fs::File::from_raw_fd(fd) };
                output.write_all(bytes)?;
                output.sync_all()?;
                // When reopening for fs-verity, check that the name still indicates the
                // created file, not the changed one.
                let written = output.metadata()?;
                // The sync file does not guarantee the stability of its name in the
                // directory.
                if unsafe { libc::fsync(parent_fd) } < 0 {
                    return Err(io::Error::last_os_error().into());
                }
                // A read-only file is suitable for subsequent fs-verity.
                let fd = unsafe {
                    libc::openat(
                        parent_fd,
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error().into());
                }
                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                let reopened = file.metadata()?;
                if !same_staged_file(&written, &reopened) {
                    return Err(ArchiveError::Unsafe);
                }
                // Prior to comparison, the original inode is held by the descriptor and
                // cannot be reused for the changed file.
                drop(output);
                files.insert(path.clone(), file.into());
            }
        }
    }
    Ok(files)
}
