use super::super::signature_inputs::SignatureCheckedArtifact;
use super::super::verified_installation::InstallationMetadataDecoder;
use super::ArchiveError;
use super::CheckedArchive;
use super::safe_path;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::unix::fs::FileExt;

const ARCHIVE_MAX: u64 = 256 * 1024 * 1024;

const FILE_MAX: u64 = 128 * 1024 * 1024;

const FILES_MAX: usize = 4096;

const METADATA_MAX: usize = 65_536;

pub(super) fn inspect_bytes(
    bytes: &[u8],
    decoder: &dyn InstallationMetadataDecoder,
    profile: &str,
) -> Result<CheckedArchive, ArchiveError> {
    if bytes.len() as u64 > ARCHIVE_MAX
        || bytes.len() < 1024
        || !bytes.len().is_multiple_of(512)
        || bytes[bytes.len() - 1024..].iter().any(|byte| *byte != 0)
    {
        return Err(ArchiveError::Limit);
    }
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut names = BTreeSet::new();
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for item in archive.entries()?.raw(true) {
        let mut entry = item?;
        let kind = entry.header().entry_type();
        let directory = kind.is_dir();
        if !directory && !kind.is_file() {
            return Err(ArchiveError::Unsafe);
        }
        let name = safe_path(&entry.path_bytes(), directory)?;
        if !names.insert(name.clone()) || names.len() > FILES_MAX {
            return Err(ArchiveError::Unsafe);
        }
        let size = entry.size();
        if (directory && size != 0) || size > FILE_MAX {
            return Err(ArchiveError::Limit);
        }
        total = total.checked_add(size).ok_or(ArchiveError::Limit)?;
        if total > ARCHIVE_MAX {
            return Err(ArchiveError::Limit);
        }
        if !directory {
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            if content.len() as u64 != size {
                return Err(ArchiveError::Unsafe);
            }
            files.insert(name, content);
        }
    }
    // The archive should not hide the second sequence after the zero block.
    let offset =
        usize::try_from(archive.into_inner().position()).map_err(|_| ArchiveError::Limit)?;
    if offset > bytes.len() || bytes[offset..].iter().any(|byte| *byte != 0) {
        return Err(ArchiveError::Unsafe);
    }
    // You can’t think of the same path as both a file and a parent directory.
    for name in &names {
        let mut parent = name.as_str();
        while let Some((head, _)) = parent.rsplit_once('/') {
            if files.contains_key(head) {
                return Err(ArchiveError::Unsafe);
            }
            parent = head;
        }
    }
    let manifest = files
        .get("package.toml")
        .filter(|b| b.len() <= METADATA_MAX)
        .ok_or(ArchiveError::InvalidMetadata)?;
    let manifest = std::str::from_utf8(manifest).map_err(|_| ArchiveError::InvalidMetadata)?;
    let manifest = decoder
        .manifest(manifest, profile)
        .map_err(|_| ArchiveError::InvalidMetadata)?;
    safe_path(manifest.executable.as_bytes(), false)?;
    safe_path(manifest.profile_path.as_bytes(), false)?;
    if !files.contains_key(&manifest.executable) {
        return Err(ArchiveError::InvalidMetadata);
    }
    let data = files
        .get(&manifest.profile_path)
        .filter(|b| b.len() <= METADATA_MAX)
        .ok_or(ArchiveError::InvalidMetadata)?;
    let data = std::str::from_utf8(data).map_err(|_| ArchiveError::InvalidMetadata)?;
    if decoder
        .profile(data)
        .map_err(|_| ArchiveError::InvalidMetadata)?
        != profile
    {
        return Err(ArchiveError::InvalidMetadata);
    }
    Ok(CheckedArchive { files })
}

/// Reads only the transmitted verified image, without repeating the resolution of the path.
pub(in crate::discovery) fn inspect(
    artifact: &SignatureCheckedArtifact,
    decoder: &dyn InstallationMetadataDecoder,
    profile: &str,
) -> Result<CheckedArchive, ArchiveError> {
    let fd: BorrowedFd<'_> = artifact.as_fd();
    // SAFETY: duplicate the actual descriptor returned by the installer.
    let dup = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if dup < 0 {
        return Err(ArchiveError::Io(io::Error::last_os_error()));
    }
    // SAFETY: fcntl has created a new owned descriptor.
    let file = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(dup) };
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 16384];
    let mut offset = 0u64;
    loop {
        let count = file.read_at(&mut buffer, offset)?;
        if count == 0 {
            break;
        }
        offset = offset
            .checked_add(count as u64)
            .ok_or(ArchiveError::Limit)?;
        if offset > ARCHIVE_MAX {
            return Err(ArchiveError::Limit);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    inspect_bytes(&bytes, decoder, profile)
}
