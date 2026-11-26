use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::Duration;

use super::{gate_channel::RELEASE, read_until::read_until};

pub(crate) struct GateReceiver(pub(super) OwnedFd);

impl GateReceiver {
    pub(in crate::process) fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.0.as_raw_fd()
    }
    pub(crate) fn from_owned(fd: OwnedFd) -> Self {
        Self(fd)
    }

    pub(crate) fn into_owned(self) -> OwnedFd {
        self.0
    }

    /// Breaking, someone else's byte and expiration of the term do not allow execution.
    pub(crate) fn wait(self, timeout: Duration) -> io::Result<()> {
        let mut byte = [0];
        if read_until(&self.0, &mut byte, timeout)? != 1 || byte[0] != RELEASE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "gate closed or invalid",
            ));
        }
        Ok(())
    }
}
