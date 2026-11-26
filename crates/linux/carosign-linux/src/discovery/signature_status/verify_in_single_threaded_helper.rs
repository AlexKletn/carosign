use super::SignatureRefusal;
use super::VerificationError;
use super::fingerprint;
use super::wait_and_check;

/// Runs a fixed file of the checker above the already open entrances.
///
/// This is just a cryptographic verification mechanism: it does not confirm the origin of the
/// cryptographic verification. `gpgv`Its libraries, key sets, or immutability of inputs.
/// Without a verified installer and control of these objects, the result cannot be used as a
/// permit for the installation or launch of the provider.
///
/// # Safety
/// Call only in a separate single-threaded assistant: for a time `spawn` The caller must
/// ensure that the other flows do not create processes or change open inputs.
/// All other open FDs starting with 3 receive `FD_CLOEXEC`.
#[cfg(test)]
pub(in crate::discovery) unsafe fn verify_in_single_threaded_helper(
    verifier: std::os::fd::BorrowedFd<'_>,
    artifact: std::os::fd::BorrowedFd<'_>,
    signature: std::os::fd::BorrowedFd<'_>,
    keyring: std::os::fd::BorrowedFd<'_>,
    approved_primary: &str,
    timeout: std::time::Duration,
) -> Result<(), VerificationError> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::process::{Command, Stdio};

    if !fingerprint(approved_primary) || timeout.is_zero() {
        return Err(VerificationError::Signature(
            SignatureRefusal::InvalidFingerprint,
        ));
    }
    for fd in [verifier, artifact, signature, keyring] {
        // SAFETY: Check the type of valid borrowed descriptor.
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd.as_raw_fd(), &mut stat) } != 0 {
            return Err(VerificationError::Io(std::io::Error::last_os_error()));
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(VerificationError::Io(
                std::io::ErrorKind::InvalidInput.into(),
            ));
        }
    }
    // A single-threaded assistant must not pass other people's FDs to gpgv, even if they were
    // opened without CLOEXEC.The original FDs remain available to the assistant; only three
    // duplicates below are allowed for inheritance.
    const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
    // SAFETY: close_range Changes the FD table flags of the current assistant only.
    if unsafe { libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, CLOSE_RANGE_CLOEXEC) } != 0 {
        return Err(VerificationError::Io(std::io::Error::last_os_error()));
    }
    let mut files: Vec<OwnedFd> = Vec::with_capacity(3);
    for fd in [artifact, signature, keyring] {
        // SAFETY: A single assistant thread temporarily creates an inherited FD.
        let dup = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD, 3) };
        if dup < 0 {
            return Err(VerificationError::Io(std::io::Error::last_os_error()));
        }
        // SAFETY: fcntl has returned a new descriptor we own.
        files.push(unsafe { OwnedFd::from_raw_fd(dup) });
    }
    let path = |fd: &OwnedFd| format!("/proc/self/fd/{}", fd.as_raw_fd());
    // exec allows a path to close CLOEXEC, so an open file cannot be replaced by renaming the
    // path between validation and launch.
    let child = Command::new(format!("/proc/self/fd/{}", verifier.as_raw_fd()))
        .env_clear()
        .arg("--homedir")
        .arg("/nonexistent")
        .arg("--status-fd=1")
        .arg("--keyring")
        .arg(path(&files[2]))
        .arg(path(&files[1]))
        .arg(path(&files[0]))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(VerificationError::Io)?;
    drop(files);
    wait_and_check(child, approved_primary, timeout)
}
