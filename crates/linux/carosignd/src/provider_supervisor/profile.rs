use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use carosign_linux::policy::{PolicyWatchState, ProtectedPolicySnapshot};

use super::ProviderIdentity;

pub(super) fn parse_extra_resource(value: &str) -> io::Result<SystemResource> {
    let mut parts = value.splitn(3, ':');
    let id = parse_id(parts.next().unwrap_or_default())?;
    let role = match parts.next() {
        Some("device") => carosign_protocol::carosign::provider::v1::FdRole::Device,
        Some("config") => carosign_protocol::carosign::provider::v1::FdRole::Config,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "неверная роль ресурса",
            ));
        }
    };
    let path = PathBuf::from(parts.next().unwrap_or_default());
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "неверный путь ресурса",
        ));
    }
    Ok(SystemResource { path, role, id })
}

pub(crate) fn parse_id(value: &str) -> io::Result<[u8; 16]> {
    if value.len() != 32 || !value.is_ascii() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "неверный идентификатор",
        ));
    }
    let mut result = [0; 16];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        };
        result[index] = digit(chunk[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "неверный идентификатор"))?
            * 16
            + digit(chunk[1]).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "неверный идентификатор")
            })?;
    }
    if result.iter().all(|byte| *byte == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "пустой идентификатор",
        ));
    }
    Ok(result)
}

pub(super) fn protected_executable(path: &Path) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "путь не абсолютный",
        ));
    }
    let canonical = path.canonicalize()?;
    let mut component = Some(canonical.as_path());
    let mut executable = true;
    while let Some(path) = component {
        let meta = std::fs::metadata(path)?;
        if meta.uid() != 0
            || meta.mode() & 0o022 != 0
            || (executable && (!meta.is_file() || meta.mode() & 0o111 == 0))
            || (!executable && !meta.is_dir())
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "путь не защищён",
            ));
        }
        executable = false;
        component = path.parent();
    }
    Ok(canonical)
}

mod provider_profile;
pub use provider_profile::ProviderProfile;
mod system_resource;
pub(super) use system_resource::SystemResource;
