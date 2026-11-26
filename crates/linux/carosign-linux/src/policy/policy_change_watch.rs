use super::PolicyWatchState;
use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

/// Observes the file, the direct directory and its ancestors before downloading the image.
/// Changing the file itself, its name or ancestors prohibits further work.
/// The owner and trusted root of the path checks the calling system composition.
pub struct PolicyChangeWatch {
    fd: OwnedFd,
    changed: bool,
    watched_names: std::collections::HashMap<libc::c_int, Vec<u8>>,
}

impl PolicyChangeWatch {
    /// Open the surveillance before reading the picture.
    pub fn new(path: &Path) -> io::Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "путь политики должен быть абсолютным и без переходов вверх",
            ));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "нет каталога политики"))?;
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "нет файла политики"))?
            .as_bytes()
            .to_vec();
        let raw = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let parent_mask = libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO
            | libc::IN_ATTRIB
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF;
        let file_mask = libc::IN_MODIFY
            | libc::IN_ATTRIB
            | libc::IN_CLOSE_WRITE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF;
        // Root-to-sheet surveillance doesn't unnoticed the renaming of an ancestor that's
        // already been passed, and character references are forbidden, because otherwise the
        // wrong directory chain would be tracked.
        let mut ancestor = PathBuf::from("/");
        let ancestor_mask = libc::IN_ATTRIB | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF;
        let mut watched_names = std::collections::HashMap::new();
        let mut previous_watch = add_watch(
            &fd,
            &ancestor,
            ancestor_mask | libc::IN_ONLYDIR | libc::IN_DONT_FOLLOW,
        )?;
        for component in parent.components() {
            match component {
                Component::RootDir => continue,
                Component::Normal(name) => {
                    watched_names.insert(previous_watch, name.as_bytes().to_vec());
                    ancestor.push(name);
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "неверный путь политики",
                    ));
                }
            }
            previous_watch = add_watch(
                &fd,
                &ancestor,
                ancestor_mask | libc::IN_ONLYDIR | libc::IN_DONT_FOLLOW,
            )?;
        }
        let parent_watch = add_watch(&fd, parent, parent_mask | libc::IN_ONLYDIR)?;
        watched_names.insert(parent_watch, name);
        // Check after installing the surveillance of the catalog: the name change after this
        // check will leave the event in the queue.
        if !std::fs::symlink_metadata(path)?.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ожидался обычный файл политики",
            ));
        }
        add_watch(&fd, path, file_mask | libc::IN_DONT_FOLLOW)?;
        Ok(Self {
            fd,
            changed: false,
            watched_names,
        })
    }

    /// Check immediately after reading and before each use of the photo.
    /// Changing the file, its name or ancestors permanently invalidates the image.
    pub fn check(&mut self) -> io::Result<PolicyWatchState> {
        if self.changed {
            return Ok(PolicyWatchState::Invalidated);
        }
        let mut buffer = [0u8; 4096];
        loop {
            let n = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                )
            };
            if n > 0 {
                let mut offset = 0;
                let size = n as usize;
                while offset < size {
                    let header = std::mem::size_of::<libc::inotify_event>();
                    if size - offset < header {
                        self.changed = true;
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "неполное событие политики",
                        ));
                    }
                    // SAFETY: Header length checked, read_unaligned Allows for alignment.
                    let event = unsafe {
                        std::ptr::read_unaligned(
                            buffer.as_ptr().add(offset).cast::<libc::inotify_event>(),
                        )
                    };
                    let end = offset
                        .checked_add(header)
                        .and_then(|v| v.checked_add(event.len as usize));
                    let Some(end) = end.filter(|end| *end <= size) else {
                        self.changed = true;
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "неполное имя события политики",
                        ));
                    };
                    let name = &buffer[offset + header..end];
                    let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];
                    if name.is_empty()
                        || self
                            .watched_names
                            .get(&event.wd)
                            .is_none_or(|expected| *expected == name)
                    {
                        self.changed = true;
                        return Ok(PolicyWatchState::Invalidated);
                    }
                    offset = end;
                }
                continue;
            }
            if n == 0 {
                self.changed = true;
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "наблюдение политики потеряно",
                ));
            }
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if err.kind() == io::ErrorKind::WouldBlock {
                return Ok(PolicyWatchState::Unchanged);
            }
            self.changed = true;
            return Err(err);
        }
    }
}

fn add_watch(fd: &OwnedFd, path: &Path, mask: u32) -> io::Result<libc::c_int> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "нулевой байт в пути"))?;
    let watch = unsafe { libc::inotify_add_watch(fd.as_raw_fd(), path.as_ptr(), mask) };
    if watch < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(watch)
}
