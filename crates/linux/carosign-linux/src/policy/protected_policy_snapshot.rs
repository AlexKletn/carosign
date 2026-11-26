use super::PolicyChangeWatch;
use super::PolicyWatchState;
use crate::sys;
use std::io;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::Component;
use std::path::Path;

/// Snapshot of the protected file and observation set before reading it.
///
/// Path protection does not confirm the correctness of the rules and does not issue
/// permission.
/// The service is obliged to analyze the rules and verify the identity of the caller.
pub struct ProtectedPolicySnapshot {
    contents: Vec<u8>,
    watch: PolicyChangeWatch,
}

impl ProtectedPolicySnapshot {
    /// Reads root-owned file on root-owned path without links and writing by outsiders.
    /// Any change during reading or failure of the kernel returns the error.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut watch = PolicyChangeWatch::new(path)?;
        let mut components = path.components();
        if components.next() != Some(Component::RootDir) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ожидался абсолютный путь",
            ));
        }
        let mut current = sys::open_launch_candidate("/").map_err(kernel_error)?;
        check_protected(&current, true)?;
        let mut components = components.peekable();
        let mut found_file = false;
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "недопустимый компонент пути",
                ));
            };
            let next = sys::open_launch_component(&current, name).map_err(kernel_error)?;
            let directory = components.peek().is_some();
            check_protected(&next, directory)?;
            current = next;
            found_file = !directory;
        }
        if !found_file {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "не указан файл политики",
            ));
        }
        let pinned = sys::launch_metadata(&current).map_err(kernel_error)?;
        let readable = sys::reopen_pinned_regular(&current).map_err(kernel_error)?;
        if sys::launch_metadata(&readable).map_err(kernel_error)? != pinned {
            return Err(io::Error::other("подменён файл политики"));
        }
        const MAX_POLICY_BYTES: u64 = 64 * 1024;
        let mut contents = Vec::new();
        std::fs::File::from(readable)
            .take(MAX_POLICY_BYTES + 1)
            .read_to_end(&mut contents)?;
        if contents.len() > MAX_POLICY_BYTES as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "файл политики слишком велик",
            ));
        }
        if sys::launch_metadata(&current).map_err(kernel_error)? != pinned
            || watch.check()? != PolicyWatchState::Unchanged
        {
            return Err(io::Error::other("файл политики изменился при чтении"));
        }
        Ok(Self { contents, watch })
    }

    /// The image and the one-time observer are transmitted together to the system owner.
    /// Before use and after analysis, the observer must be checked again.
    pub fn into_parts(self) -> (Vec<u8>, PolicyChangeWatch) {
        (self.contents, self.watch)
    }
}

fn kernel_error(error: sys::SysError) -> io::Error {
    io::Error::from_raw_os_error(error.errno)
}

fn check_protected(fd: &OwnedFd, directory: bool) -> io::Result<()> {
    let metadata = sys::launch_metadata(fd).map_err(kernel_error)?;
    let kind = if directory {
        libc::S_IFDIR
    } else {
        libc::S_IFREG
    };
    if metadata.identity.uid != 0
        || metadata.mode & libc::S_IFMT != kind
        || metadata.mode & (libc::S_IWGRP | libc::S_IWOTH) != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "небезопасный путь политики",
        ));
    }
    Ok(())
}
