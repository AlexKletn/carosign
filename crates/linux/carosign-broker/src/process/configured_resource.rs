use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, PathBuf};

/// A system path transmitted by a trusted supervisor.
pub struct ConfiguredResource {
    pub path: PathBuf,
    pub resource_id: [u8; 16],
    pub role: i32,
}

fn validate_resource_parents(resource: &ConfiguredResource) -> std::io::Result<()> {
    let path = &resource.path;
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(std::io::Error::other("недопустимый путь ресурса"));
    }
    let mut parent = path.parent();
    while let Some(dir) = parent {
        let meta = std::fs::symlink_metadata(dir)?;
        // In the username space, the owner of the external root may not be mapped; only the
        // immutable root is allowed in this form; all resource directories must belong to the
        // system owner.
        let unmapped_root = dir == std::path::Path::new("/")
            && meta.uid() == 65534
            && meta.mode() & 0o222 == 0
            && std::fs::read_to_string("/proc/self/uid_map")
                .ok()
                .is_some_and(|map| {
                    let fields: Vec<_> = map.split_whitespace().collect();
                    fields.len() == 3 && fields[0] == "0" && fields[1] != "0" && fields[2] == "1"
                });
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() } && !unmapped_root)
            || meta.mode() & 0o022 != 0
        {
            return Err(std::io::Error::other("каталог ресурса не защищён"));
        }
        parent = dir.parent();
    }
    Ok(())
}

pub(super) fn open_resource(resource: &ConfiguredResource) -> std::io::Result<OwnedFd> {
    validate_resource_parents(resource)?;
    let path = &resource.path;
    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .write(resource.role == 1)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
        || (resource.role == 1 && meta.mode() & libc::S_IFMT != libc::S_IFCHR)
        || (resource.role == 3
            && (meta.mode() & libc::S_IFMT != libc::S_IFREG || meta.mode() & 0o022 != 0))
    {
        return Err(std::io::Error::other("неверный системный ресурс"));
    }
    if resource.role != 3 {
        return Ok(file.into());
    }
    // Config is an immutable image, not a descriptor of the original file.
    const CONFIG_LIMIT: u64 = 16 * 1024 * 1024;
    if meta.len() > CONFIG_LIMIT {
        return Err(std::io::Error::other("конфигурация слишком велика"));
    }
    let memory = carosign_linux::sys::memfd("carosign-config")
        .map_err(|e| std::io::Error::from_raw_os_error(e.errno))?;
    let mut destination = std::fs::File::from(memory);
    let copied = std::io::copy(&mut file.take(CONFIG_LIMIT + 1), &mut destination)?;
    if copied > CONFIG_LIMIT {
        return Err(std::io::Error::other("конфигурация слишком велика"));
    }
    destination.flush()?;
    let memory: OwnedFd = destination.into();
    carosign_linux::sys::seal_read_only(&memory)
        .map_err(|e| std::io::Error::from_raw_os_error(e.errno))?;
    let readonly = std::fs::File::open(format!("/proc/self/fd/{}", memory.as_raw_fd()))?;
    Ok(readonly.into())
}

/// Owner does not assign a resource: FD must refer to the same secure node that is installed
/// in the profile.
pub(super) fn validate_owned_resource(
    resource: &ConfiguredResource,
    fd: &OwnedFd,
) -> std::io::Result<()> {
    validate_resource_parents(resource)?;
    let path = std::fs::symlink_metadata(&resource.path)?;
    let descriptor = std::fs::File::from(fd.try_clone()?).metadata()?;
    // SAFETY: fd belongs to the caller and is alive for the time fcntl.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    let access = flags & libc::O_ACCMODE;
    let kind = path.mode() & libc::S_IFMT;
    if flags < 0
        || flags & libc::O_PATH != 0
        || path.file_type().is_symlink()
        || (path.uid() != 0 && path.uid() != unsafe { libc::geteuid() })
        || path.dev() != descriptor.dev()
        || path.ino() != descriptor.ino()
        || path.rdev() != descriptor.rdev()
        || (resource.role == 1 && (kind != libc::S_IFCHR || access != libc::O_RDWR))
        || (resource.role == 3
            && (kind != libc::S_IFREG || access != libc::O_RDONLY || path.mode() & 0o022 != 0))
    {
        return Err(std::io::Error::other(
            "дескриптор не совпал с системным ресурсом",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "configured_resource.tests.rs"]
mod tests;
