use std::os::fd::OwnedFd;

use super::ArchiveError;

/// Verifies the file attached after fs-verity with the signed archive image.
/// Inode reconciliation and length alone are not enough: the contents of the same inode may
/// have changed before the protection is turned on. Reading does not change the position of
/// the descriptor.
pub(in crate::discovery) fn matches_signed_bytes(
    file: &OwnedFd,
    expected: &[u8],
) -> Result<bool, ArchiveError> {
    use std::os::unix::fs::FileExt;
    let input = std::fs::File::from(file.try_clone()?);
    if input.metadata()?.len() != expected.len() as u64 {
        return Ok(false);
    }
    let mut buffer = [0u8; 16384];
    let mut offset = 0usize;
    while offset < expected.len() {
        let take = buffer.len().min(expected.len() - offset);
        let count = input.read_at(&mut buffer[..take], offset as u64)?;
        if count == 0 || buffer[..count] != expected[offset..offset + count] {
            return Ok(false);
        }
        offset += count;
    }
    Ok(true)
}
