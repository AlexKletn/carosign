use super::VerificationError;
use super::check;
use super::reap::Reap;

pub(super) fn wait_and_check(
    child: std::process::Child,
    approved_primary: &str,
    timeout: std::time::Duration,
) -> Result<(), VerificationError> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::time::Instant;

    let mut child = Reap(child);
    let mut out = child
        .0
        .stdout
        .take()
        .ok_or_else(|| VerificationError::Io(std::io::Error::other("нет канала отчёта")))?;
    // SAFETY: We only change the attribute of the end of the channel.
    let flags = unsafe { libc::fcntl(out.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(out.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(VerificationError::Io(std::io::Error::last_os_error()));
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(VerificationError::Timeout)?;
    let mut status = Vec::new();
    let mut done = None;
    let mut eof = false;
    while done.is_none() || !eof {
        let mut buf = [0u8; 4096];
        match out.read(&mut buf) {
            Ok(0) => eof = true,
            Ok(n) => {
                if status.len() + n > 65_536 {
                    return Err(VerificationError::OutputLimit);
                }
                status.extend_from_slice(&buf[..n]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            // The time and state of the process are checked even when interrupted: the signal
            // flow should not prolong the check indefinitely.
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(VerificationError::Io(error)),
        }
        if done.is_none() {
            done = child.0.try_wait().map_err(VerificationError::Io)?;
        }
        if done.is_some() && eof {
            break;
        }
        if Instant::now() >= deadline {
            return Err(VerificationError::Timeout);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    check(
        done.expect("проверено выше").success(),
        &status,
        approved_primary,
    )
    .map_err(VerificationError::Signature)
}
