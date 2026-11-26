use carosign_protocol::carosign::provider::v1 as provider;
use std::os::fd::RawFd;

#[cfg(feature = "ssh-mock")]
pub(super) fn wait_ssh_decision(fd: RawFd) -> Result<Option<provider::VerificationStatus>, String> {
    wait_ssh_decision_at(fd, "/run/carosign-mock/control", 0)
}

#[cfg(feature = "ssh-mock")]
fn wait_ssh_decision_at(
    fd: RawFd,
    path: &str,
    owner: u32,
) -> Result<Option<provider::VerificationStatus>, String> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
    let mut fifo = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "mock control FIFO unavailable")?;
    let metadata = fifo
        .metadata()
        .map_err(|_| "mock control FIFO stat failed")?;
    if !metadata.file_type().is_fifo() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 {
        return Err("mock control FIFO is not root-owned and protected".into());
    }
    eprintln!("MOCK: waiting for SSH decision: match, no-match, no-sample");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(55);
    let mut input = Vec::new();
    loop {
        if std::time::Instant::now() >= deadline {
            return Ok(Some(provider::VerificationStatus::VerifyNoUsableSample));
        }
        let mut polls = [
            libc::pollfd {
                fd,
                events: libc::POLLIN | libc::POLLRDHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: fifo.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: Both descriptors and arrays live until the poll returns.
        let result = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, 200) };
        if result < 0 {
            if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err("mock poll failed".into());
        }
        if polls[0].revents & (libc::POLLIN | libc::POLLRDHUP | libc::POLLHUP | libc::POLLERR) != 0
        {
            return Ok(None);
        }
        if polls[1].revents & libc::POLLIN != 0 {
            let mut bytes = [0u8; 64];
            match fifo.read(&mut bytes) {
                Ok(n) => input.extend_from_slice(&bytes[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return Err("mock decision read failed".into()),
            }
            if input.len() > 64 {
                return Err("mock decision too long".into());
            }
            if input.contains(&b'\n') {
                return match input.as_slice() {
                    b"match\n" => Ok(Some(provider::VerificationStatus::VerifyMatched)),
                    b"no-match\n" => Ok(Some(provider::VerificationStatus::VerifyNotMatched)),
                    b"no-sample\n" => Ok(Some(provider::VerificationStatus::VerifyNoUsableSample)),
                    _ => Err("invalid mock decision".into()),
                };
            }
        }
        // FIFO without a writer is constantly reporting to HUP; limiting the frequency of the
        // survey.
        if polls[1].revents & libc::POLLHUP != 0 {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}

#[path = "ssh_decision.tests.rs"]
#[cfg(all(test, feature = "ssh-mock"))]
mod ssh_mock_tests;
