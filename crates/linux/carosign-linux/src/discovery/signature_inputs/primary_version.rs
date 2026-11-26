use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd};
use std::os::unix::fs::FileExt;

use super::{InputError, signature_inputs::KEYRING_MAX};

/// Finds the main key packet from a self-computed print.
/// gpgv separately confirms the signature with this key from the same keyring image.
/// Indefinite and partial packet lengths are deliberately rejected.
pub(super) fn primary_version(
    keyring: BorrowedFd<'_>,
    fingerprint: &str,
) -> Result<u8, InputError> {
    let duplicate = unsafe { libc::fcntl(keyring.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if duplicate < 0 {
        return Err(InputError::Io(io::Error::last_os_error()));
    }
    let file = unsafe { std::fs::File::from_raw_fd(duplicate) };
    let size = file.metadata()?.len();
    if size > KEYRING_MAX {
        return Err(InputError::TooLarge);
    }
    let mut bytes = vec![0; size as usize];
    file.read_exact_at(&mut bytes, 0)?;
    let mut offset = 0usize;
    let mut version = None;
    while offset < bytes.len() {
        let header = bytes[offset];
        offset += 1;
        if header & 0x80 == 0 {
            return Err(InputError::InvalidFile);
        }
        let (tag, length) = if header & 0x40 != 0 {
            let tag = header & 0x3f;
            let first = *bytes.get(offset).ok_or(InputError::InvalidFile)?;
            offset += 1;
            let length = match first {
                0..=191 => first as usize,
                192..=223 => {
                    let second = *bytes.get(offset).ok_or(InputError::InvalidFile)?;
                    offset += 1;
                    ((first as usize - 192) << 8) + second as usize + 192
                }
                255 => {
                    let raw: [u8; 4] = bytes
                        .get(offset..offset + 4)
                        .ok_or(InputError::InvalidFile)?
                        .try_into()
                        .map_err(|_| InputError::InvalidFile)?;
                    offset += 4;
                    u32::from_be_bytes(raw) as usize
                }
                _ => return Err(InputError::InvalidFile),
            };
            (tag, length)
        } else {
            let tag = (header >> 2) & 0x0f;
            let n = match header & 3 {
                0 => 1,
                1 => 2,
                2 => 4,
                _ => return Err(InputError::InvalidFile),
            };
            let raw = bytes
                .get(offset..offset + n)
                .ok_or(InputError::InvalidFile)?;
            offset += n;
            let length = raw.iter().fold(0usize, |acc, &b| (acc << 8) | b as usize);
            (tag, length)
        };
        let end = offset
            .checked_add(length)
            .filter(|&v| v <= bytes.len())
            .ok_or(InputError::InvalidFile)?;
        if tag == 6 {
            if length < 6 {
                return Err(InputError::InvalidFile);
            }
            let found = bytes[offset];
            let body = &bytes[offset..end];
            let digest = match found {
                4 => {
                    let length = u16::try_from(length).map_err(|_| InputError::InvalidFile)?;
                    let mut hash = Sha1::new();
                    hash.update([0x99]);
                    hash.update(length.to_be_bytes());
                    hash.update(body);
                    hash.finalize().to_vec()
                }
                5 | 6 => {
                    let length = u32::try_from(length).map_err(|_| InputError::InvalidFile)?;
                    let mut hash = Sha256::new();
                    hash.update([if found == 5 { 0x9a } else { 0x9b }]);
                    hash.update(length.to_be_bytes());
                    hash.update(body);
                    hash.finalize().to_vec()
                }
                _ => return Err(InputError::InvalidFile),
            };
            let calculated: String = digest.iter().map(|b| format!("{b:02X}")).collect();
            if calculated == fingerprint && version.replace(found).is_some() {
                return Err(InputError::InvalidFile);
            }
        }
        if matches!(tag, 5 | 7) {
            return Err(InputError::InvalidFile);
        }
        offset = end;
    }
    version.ok_or(InputError::InvalidFile)
}
