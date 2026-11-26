use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

pub(super) fn read_until(fd: &OwnedFd, bytes: &mut [u8], timeout: Duration) -> io::Result<usize> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(io::ErrorKind::InvalidInput)?;
    let mut count = 0;
    while count < bytes.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::from(io::ErrorKind::TimedOut));
        }
        let millis = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let mut pollfd = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd is valid; the number of elements is one.
        let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if result == 0 {
            continue;
        }
        // SAFETY: The slice is writable, and the write is bounded by its length.
        let n = unsafe {
            libc::read(
                fd.as_raw_fd(),
                bytes[count..].as_mut_ptr().cast(),
                bytes.len() - count,
            )
        };
        if n == 0 {
            return Ok(count);
        }
        if n < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        count += n as usize;
    }
    Ok(count)
}
