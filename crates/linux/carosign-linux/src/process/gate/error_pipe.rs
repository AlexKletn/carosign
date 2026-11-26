use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

use super::{ErrorReader, ErrorWriter};

/// Closed to exec channel of notification of refusal.
pub(crate) struct ErrorPipe {
    parent: OwnedFd,
    child: OwnedFd,
}

impl ErrorPipe {
    pub(crate) fn new() -> io::Result<Self> {
        let (parent, child) = pipe()?;
        Ok(Self { parent, child })
    }

    pub(crate) fn into_parts(self) -> (ErrorReader, ErrorWriter) {
        (ErrorReader(self.parent), ErrorWriter(self.child))
    }
}

fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [-1; 2];
    // SAFETY: The array contains space for two returnable descriptors.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: The successful pipe2 has created two independent FDs.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

#[cfg(test)]
#[path = "error_pipe.tests.rs"]
mod tests;
