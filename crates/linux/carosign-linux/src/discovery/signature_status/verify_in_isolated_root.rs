use super::SignatureRefusal;
use super::VerificationError;
use super::cap_data::CapData;
use super::cap_header::CapHeader;
use super::fingerprint;
use super::wait_and_check;

/// The tester starts in the already assembled and closed root of the assistant.
///
/// # Safety
/// The root and the paths inside it must be fully prepared in a separate single-threaded
/// assistant, without the possibility of replacing files with strangers.
pub(in crate::discovery) unsafe fn verify_in_isolated_root(
    root: std::os::fd::BorrowedFd<'_>,
    approved_primary: &str,
    timeout: std::time::Duration,
) -> Result<(), VerificationError> {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    if !fingerprint(approved_primary) || timeout.is_zero() {
        return Err(VerificationError::Signature(
            SignatureRefusal::InvalidFingerprint,
        ));
    }
    const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
    // SAFETY: It is called only in a separate single-threaded assistant.
    if unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, CLOSE_RANGE_CLOEXEC) } != 0 {
        return Err(VerificationError::Io(std::io::Error::last_os_error()));
    }
    let root_fd = root.as_raw_fd();
    let mut command = Command::new("/usr/bin/gpgv");
    command
        .env_clear()
        .args([
            "--homedir",
            "/nonexistent",
            "--status-fd=1",
            "--keyring",
            "/inputs/trusted.gpg",
            "/inputs/artifact.sig",
            "/inputs/artifact",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: After fork, only system calls with FD retained are executed.
    unsafe {
        command.pre_exec(move || {
            if libc::fchdir(root_fd) != 0
                || libc::chroot(c".".as_ptr()) != 0
                || libc::chdir(c"/".as_ptr()) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            // UID 0 should not restore capabilities in exec; then remove permitted,
            // inherited, and valid capabilities.
            if libc::prctl(libc::PR_SET_SECUREBITS, 3, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let header = CapHeader {
                version: 0x2008_0522,
                pid: 0,
            };
            let data = [
                CapData {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
                CapData {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
            ];
            if libc::syscall(libc::SYS_capset, &header, data.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // We do not allow the executable file to gain new authority.
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(VerificationError::Io)?;
    wait_and_check(child, approved_primary, timeout)
}
