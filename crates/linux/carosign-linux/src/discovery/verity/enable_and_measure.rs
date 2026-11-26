use super::EnableArg;
use super::SHA256;
use super::measure;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::BorrowedFd;

const ENABLE: libc::c_ulong = 0x4080_6685;

/// Turns on kernel protection and returns the measurement of the retained file. The value is
/// not a confirmation of origin and does not publish the installation record.
pub(in crate::discovery) fn enable_and_measure(fd: BorrowedFd<'_>) -> io::Result<[u8; 32]> {
    let args = EnableArg {
        version: 1,
        hash_algorithm: u32::from(SHA256),
        block_size: 0,
        salt_size: 0,
        salt_ptr: 0,
        sig_size: 0,
        reserved1: 0,
        sig_ptr: 0,
        reserved2: [0; 11],
    };
    // SAFETY: The structure corresponds to Linux UAPI and lives until the return of ioctl.
    if unsafe { libc::ioctl(fd.as_raw_fd(), ENABLE, &args) } < 0 {
        return Err(io::Error::last_os_error());
    }
    measure(fd)
}
