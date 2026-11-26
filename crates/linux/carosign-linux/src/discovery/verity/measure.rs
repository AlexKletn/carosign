use super::Digest;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;

// Linux fsverity.h: _IOWR('f', 134, struct fsverity_digest).
pub(super) const MEASURE: libc::c_ulong = 0xc004_6686;

pub(super) const SHA256: u16 = 1;

/// Checks the measurement of an already open file; the absence of fs-verity is a failure.
pub(super) fn measure(fd: BorrowedFd<'_>) -> io::Result<[u8; 32]> {
    let mut digest = Digest {
        algorithm: 0,
        size: 32,
        bytes: [0; 32],
    };
    // SAFETY: The kernel receives a header buffer and 32 bytes with the correct size.
    if unsafe { libc::ioctl(fd.as_raw_fd(), MEASURE, &mut digest) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if digest.algorithm != SHA256 || digest.size != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "неподдерживаемое измерение fs-verity",
        ));
    }
    Ok(digest.bytes)
}

pub(crate) fn matches_approved_digest(fd: BorrowedFd<'_>, approved: &[u8; 32]) -> io::Result<bool> {
    Ok(measure(fd)? == *approved)
}
