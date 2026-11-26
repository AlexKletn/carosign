use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

use super::{GateReceiver, GateSender};

pub(super) const RELEASE: u8 = 0xa5;

/// Parental and daughter ends of a one-time permit.
pub(crate) struct Gate {
    parent: OwnedFd,
    child: OwnedFd,
}

impl Gate {
    pub(crate) fn new() -> io::Result<Self> {
        let mut fds = [-1; 2];
        // SAFETY: The array contains space for a pair of new descriptors.
        if unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: A successful socketpair passed both descriptors to the caller.
        let (child, parent) =
            unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        Ok(Self { parent, child })
    }

    pub(crate) fn into_parts(self) -> (GateSender, GateReceiver) {
        (GateSender(self.parent), GateReceiver(self.child))
    }
}
