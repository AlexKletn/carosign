use std::io;
use std::os::fd::{AsRawFd, OwnedFd};

pub(crate) struct ErrorWriter(pub(super) OwnedFd);

impl ErrorWriter {
    pub(crate) fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.0.as_raw_fd()
    }
    #[cfg(test)]
    pub(crate) fn from_owned(fd: OwnedFd) -> Self {
        Self(fd)
    }

    #[cfg(test)]
    pub(crate) fn into_owned(self) -> OwnedFd {
        self.0
    }

    pub(crate) fn report(self, stage: u8, errno: i32) -> io::Result<()> {
        let mut message = [0; 5];
        message[0] = stage;
        message[1..].copy_from_slice(&errno.to_be_bytes());
        write_all(&self.0, &message)
    }
}

fn write_all(fd: &OwnedFd, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        // SAFETY: The buffer and the descriptor are valid.
        let n = unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        if n > 0 {
            bytes = &bytes[n as usize..];
        } else if n == 0 {
            return Err(io::Error::from(io::ErrorKind::WriteZero));
        } else {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    Ok(())
}
