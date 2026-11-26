use super::ArchiveError;
use super::CheckedArchive;
use super::matches_signed_bytes;
use super::stage;
use std::collections::BTreeMap;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;
use std::os::fd::OwnedFd;

/// Records the contents of each extracted file with the kernel.
/// Error at any stage prohibits the publication of the entire version.
pub(in crate::discovery) fn stage_and_measure(
    archive: &CheckedArchive,
    root: BorrowedFd<'_>,
) -> Result<BTreeMap<String, (OwnedFd, [u8; 32])>, ArchiveError> {
    use std::os::fd::AsFd;
    let files = stage(archive, root)?;
    let mut measured = BTreeMap::new();
    for (path, file) in files {
        let digest = super::super::verity::enable_and_measure(file.as_fd())?;
        let expected = archive.files.get(&path).ok_or(ArchiveError::Unsafe)?;
        if !matches_signed_bytes(&file, expected)? {
            return Err(ArchiveError::Unsafe);
        }
        if unsafe { libc::fsync(file.as_raw_fd()) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        measured.insert(path, (file, digest));
    }
    Ok(measured)
}
