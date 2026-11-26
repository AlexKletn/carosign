use std::io;
use std::os::fd::{AsRawFd, OwnedFd};

use super::gate_channel::RELEASE;

pub(crate) struct GateSender(pub(super) OwnedFd);

impl GateSender {
    /// One resolution; closing without it leaves the descendant blocked.
    pub(crate) fn release(self) -> io::Result<()> {
        loop {
            // SAFETY: One byte is sent over a socket belonging to it. MSG_NOSIGNAL It does
            // not allow the lost offspring to complete with a supervisor signal.
            let sent = unsafe {
                libc::send(
                    self.0.as_raw_fd(),
                    (&RELEASE as *const u8).cast(),
                    1,
                    libc::MSG_NOSIGNAL,
                )
            };
            if sent == 1 {
                return Ok(());
            }
            if sent == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
