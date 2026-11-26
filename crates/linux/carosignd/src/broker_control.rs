//! Private management of the broker regardless of the expectation of the employee
//! events.

use carosign_protocol::carosign::broker_relay::v1::{
    Control, ControlResult, Revoke, Scope, control, control_result,
};
use prost::Message;

/// The sole owner of the channel, separated from the working supervisor.
pub(crate) struct BrokerControl(OwnedFd);

impl BrokerControl {
    pub(crate) fn new(fd: OwnedFd) -> Self {
        Self(fd)
    }

    pub(crate) fn revoke(self, scope: Scope, timeout: Duration) -> io::Result<()> {
        let request_id =
            carosign_linux::sys::random_16().map_err(|_| io::Error::other("нет ID отзыва"))?;
        revoke(self.0, scope, request_id, timeout)
    }
}
use std::{
    io,
    os::fd::{AsRawFd, OwnedFd},
    time::{Duration, Instant},
};

fn wait(fd: &OwnedFd, events: libc::c_short, deadline: Instant) -> io::Result<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(io::ErrorKind::TimedOut)?;
    let millis = remaining
        .as_millis()
        .saturating_add(1)
        .min(i32::MAX as u128) as i32;
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events,
        revents: 0,
    };
    // SAFETY: The poll owns one correct item before the return.
    match unsafe { libc::poll(&mut poll, 1, millis) } {
        0 => Err(io::ErrorKind::TimedOut.into()),
        n if n < 0 => Err(io::Error::last_os_error()),
        _ if poll.revents & events == 0 => Err(io::ErrorKind::BrokenPipe.into()),
        _ => Ok(()),
    }
}

/// The channel belongs to the supervisor; any unknown outcome renders it unusable.
pub(crate) fn revoke(
    fd: OwnedFd,
    scope: Scope,
    request_id: [u8; 16],
    timeout: Duration,
) -> io::Result<()> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(io::ErrorKind::InvalidInput)?;
    if request_id.iter().all(|byte| *byte == 0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let request = Control {
        request_id: request_id.to_vec(),
        action: Some(control::Action::Revoke(Revoke { scope: Some(scope) })),
    }
    .encode_to_vec();
    wait(&fd, libc::POLLOUT, deadline)?;
    // SAFETY: The socket and buffer remain alive until the end of the send.
    let sent = unsafe {
        libc::send(
            fd.as_raw_fd(),
            request.as_ptr().cast(),
            request.len(),
            libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
        )
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if sent as usize != request.len() {
        return Err(io::ErrorKind::WriteZero.into());
    }
    wait(&fd, libc::POLLIN, deadline)?;
    let mut bytes = vec![0u8; carosign_protocol::limits::FRAME_MAX];
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: The zero structure of msghdr is acceptable, iov lives until the end of recvmsg.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    // SAFETY: All the buffers belong to the current call.
    let count = unsafe {
        libc::recvmsg(
            fd.as_raw_fd(),
            &mut message,
            libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
        )
    };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    if count == 0 || message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    bytes.truncate(count as usize);
    let result = ControlResult::decode(bytes.as_slice()).map_err(|_| io::ErrorKind::InvalidData)?;
    if result.encode_to_vec() != bytes
        || result.request_id != request_id
        || result.result != Some(control_result::Result::ExclusiveQuiesced(true))
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

#[path = "broker_control.tests.rs"]
#[cfg(test)]
mod tests;
