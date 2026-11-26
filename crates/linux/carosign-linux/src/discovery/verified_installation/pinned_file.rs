use std::os::fd::OwnedFd;
use std::path::{Component, Path};

use crate::sys;

use super::{Refusal, approved_path::approved_path, protected::protected};

pub(super) fn pinned_file(root: &OwnedFd, path: &str, owner: u32) -> Result<OwnedFd, Refusal> {
    if !approved_path(path) {
        return Err(Refusal::UnsafePath);
    }
    let parts: Vec<_> = Path::new(path).components().collect();
    if parts.is_empty() || parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
        return Err(Refusal::UnsafePath);
    }
    let mut current = None;
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            return Err(Refusal::UnsafePath);
        };
        let next = sys::open_launch_component(current.as_ref().unwrap_or(root), name)
            .map_err(|e| Refusal::Kernel(e.errno))?;
        let metadata = sys::launch_metadata(&next).map_err(|e| Refusal::Kernel(e.errno))?;
        if !protected(&metadata, owner, index + 1 != parts.len()) {
            return Err(Refusal::UnsafeObject);
        }
        current = Some(next);
    }
    let pinned = current.ok_or(Refusal::UnsafePath)?;
    let before = sys::launch_metadata(&pinned).map_err(|e| Refusal::Kernel(e.errno))?;
    let readable = sys::reopen_pinned_regular(&pinned).map_err(|e| Refusal::Kernel(e.errno))?;
    if sys::launch_metadata(&readable).map_err(|e| Refusal::Kernel(e.errno))? != before
        || sys::launch_metadata(&pinned).map_err(|e| Refusal::Kernel(e.errno))? != before
    {
        return Err(Refusal::Changed);
    }
    Ok(readable)
}

#[cfg(test)]
#[path = "pinned_file.tests.rs"]
mod tests;
