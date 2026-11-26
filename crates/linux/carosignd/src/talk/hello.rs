use super::{Connection, Ended};

pub(super) fn wait_for_hello(
    connection: &Connection,
    timeout: std::time::Duration,
) -> Result<(), Ended> {
    use std::os::fd::AsRawFd;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(Ended::Refused("hello timeout".to_string()));
        }
        let millis = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let mut pollfd = libc::pollfd {
            fd: connection.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd indicates one valid element at the call time.
        let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
        if result > 0 {
            return Ok(());
        }
        if result == 0 {
            continue;
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
        if errno != libc::EINTR {
            return Err(Ended::Broken(errno));
        }
    }
}
