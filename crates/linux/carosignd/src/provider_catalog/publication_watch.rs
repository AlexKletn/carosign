use std::{
    collections::BTreeMap,
    ffi::CString,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
    path::Path,
    time::Duration,
};

pub(super) struct PublicationWatch {
    fd: OwnedFd,
    names: BTreeMap<i32, Vec<Vec<u8>>>,
}

impl PublicationWatch {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: inotify_init1 returns a new descriptor on success.
        let raw = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: raw is newly owned by this call.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        Ok(Self {
            fd,
            names: BTreeMap::new(),
        })
    }

    pub(super) fn add(&mut self, directory: &Path, name: &[u8]) -> io::Result<()> {
        let directory = CString::new(directory.as_os_str().as_bytes())?;
        let mask = libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO
            | libc::IN_ATTRIB
            | libc::IN_CLOSE_WRITE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF
            | libc::IN_ONLYDIR
            | libc::IN_DONT_FOLLOW;
        // SAFETY: directory is NUL terminated and fd remains owned.
        let watch =
            unsafe { libc::inotify_add_watch(self.fd.as_raw_fd(), directory.as_ptr(), mask) };
        if watch < 0 {
            let error = io::Error::last_os_error();
            return if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::NotADirectory
                    | io::ErrorKind::PermissionDenied
            ) || error.raw_os_error() == Some(libc::ELOOP)
            {
                Ok(())
            } else {
                Err(error)
            };
        }
        self.names.entry(watch).or_default().push(name.to_vec());
        Ok(())
    }

    pub(super) fn changed(&self, wait: Duration) -> io::Result<Option<bool>> {
        let mut poll = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll is writable and references the owned descriptor.
        let ready =
            unsafe { libc::poll(&mut poll, 1, wait.as_millis().min(i32::MAX as u128) as i32) };
        if ready == 0 {
            return Ok(None);
        }
        if ready < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = [0u8; 8192];
        let mut changed = None;
        for _ in 0..128 {
            // SAFETY: buffer is writable for the specified length.
            let count = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                )
            };
            if count < 0 {
                let error = io::Error::last_os_error();
                return if error.kind() == io::ErrorKind::WouldBlock {
                    Ok(changed)
                } else {
                    Err(error)
                };
            }
            if count == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            match self.decode(&buffer[..count as usize])? {
                Some(true) => return Ok(Some(true)),
                Some(false) => changed = Some(false),
                None => {}
            }
        }
        Ok(Some(true))
    }

    fn decode(&self, mut bytes: &[u8]) -> io::Result<Option<bool>> {
        let mut changed = false;
        let header = std::mem::size_of::<libc::inotify_event>();
        while !bytes.is_empty() {
            if bytes.len() < header {
                return Err(io::ErrorKind::InvalidData.into());
            }
            // SAFETY: bytes contains a complete fixed-size header. The kernel buffer may be unaligned.
            let event = unsafe {
                bytes
                    .as_ptr()
                    .cast::<libc::inotify_event>()
                    .read_unaligned()
            };
            bytes = &bytes[header..];
            let (name, rest) = bytes
                .split_at_checked(event.len as usize)
                .ok_or(io::ErrorKind::InvalidData)?;
            bytes = rest;
            if event.mask & libc::IN_Q_OVERFLOW != 0 {
                return Ok(Some(true));
            }
            let name = name.split(|byte| *byte == 0).next().unwrap_or_default();
            changed |= self.names.get(&event.wd).is_some_and(|names| {
                name.is_empty() || names.iter().any(|expected| expected == name)
            });
        }
        Ok(changed.then_some(false))
    }
}

#[cfg(test)]
#[path = "publication_watch.tests.rs"]
mod tests;
