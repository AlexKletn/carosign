use super::AllowedFd;
use super::ExecDisposition;
use std::io;

/// Closes all FDs off the list and displays the specified FD flags.
///
/// It is not atomic with respect to other streams: the caller must be a separate
/// single-threaded process and, if any error is made, immediately terminate.
/// If `close_range` Inaccessible, refusal is mandatory, bypassing `/proc` Untested code
/// cannot be executed until the function is completed.
pub(crate) fn retain_only(allowed: &[AllowedFd]) -> io::Result<()> {
    let mut sorted = allowed.to_vec();
    sorted.sort_unstable_by_key(|entry| entry.fd);
    if sorted.iter().any(|entry| entry.fd < 0)
        || sorted.windows(2).any(|pair| pair[0].fd == pair[1].fd)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid or duplicate fd",
        ));
    }
    // First, make sure that the entire list is open; errors after changing the table mean a
    // process failure, not a rollback or partial success.
    let mut flags = Vec::with_capacity(sorted.len());
    for entry in &sorted {
        // SAFETY: fcntl does not own the transmitted descriptor.
        let current = unsafe { libc::fcntl(entry.fd, libc::F_GETFD) };
        if current < 0 {
            return Err(io::Error::last_os_error());
        }
        flags.push(current);
    }
    for (entry, current) in sorted.iter().zip(flags) {
        let wanted = match entry.disposition {
            ExecDisposition::Inherit => current & !libc::FD_CLOEXEC,
            ExecDisposition::Close => current | libc::FD_CLOEXEC,
        };
        // SAFETY: fcntl only changes the flag of the open assistant descriptor.
        if unsafe { libc::fcntl(entry.fd, libc::F_SETFD, wanted) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut first = 0_u32;
    for entry in sorted {
        let fd = entry.fd as u32;
        if first < fd {
            close_range(first, fd - 1)?;
        }
        first = fd + 1;
    }
    close_range(first, u32::MAX)
}

fn close_range(first: u32, last: u32) -> io::Result<()> {
    // SAFETY: close_range It does not refer to user pointers.
    if unsafe { libc::syscall(libc::SYS_close_range, first, last, 0_u32) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
