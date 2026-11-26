use super::ExchangeError;

use std::os::fd::{AsRawFd, OwnedFd};
use std::time::{Duration, Instant};

use carosign_protocol::carosign::provider::v1::{ProviderFrame, provider_frame::Payload};
use carosign_provider_sdk::wire::Channel;
use zeroize::Zeroizing;

/// The sole owner of the channel and its serial numbers.
pub struct ProviderTransport {
    fd: OwnedFd,
    channel: Channel,
    failed: bool,
}

impl ProviderTransport {
    /// Accepts the existing channel and counters after the previous exchange.
    /// This does not confirm who created the socket or who is on the other end.
    pub fn new(fd: OwnedFd, channel: Channel) -> Result<Self, ExchangeError> {
        let mut kind: libc::c_int = 0;
        let mut length = std::mem::size_of_val(&kind) as libc::socklen_t;
        // SAFETY: Addresses are valid until getockopt returns.
        if unsafe {
            libc::getsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut libc::c_int).cast(),
                &mut length,
            )
        } != 0
            || kind != libc::SOCK_SEQPACKET
        {
            return Err(ExchangeError::InvalidSocket);
        }
        // The public pathname or abstract address contradicts the private channel contract,
        // so we check both ends: the received connection has a name on the local side, and
        // the outgoing one on the remote.
        if !unnamed_unix_address(fd.as_raw_fd(), false)
            || !unnamed_unix_address(fd.as_raw_fd(), true)
        {
            return Err(ExchangeError::InvalidSocket);
        }
        // SAFETY: The descriptor belongs to the object; CLOEXEC does not change the socket.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
        {
            return Err(ExchangeError::Io(std::io::Error::last_os_error()));
        }
        Ok(Self {
            fd,
            channel,
            failed: false,
        })
    }

    /// Sends one frame; does not support Bind with descriptors.
    /// After sending an error, its actual outcome may not be known.
    pub fn send(&mut self, frame: ProviderFrame, timeout: Duration) -> Result<(), ExchangeError> {
        if self.failed {
            return Err(ExchangeError::Closed);
        }
        if matches!(&frame.payload, Some(Payload::BindAuthenticator(bind)) if !bind.fd_slots.is_empty())
        {
            return Err(ExchangeError::DescriptorsUnsupported);
        }
        // The serialized frame may contain opaque data of the provider; erase this copy at
        // any output, including sending error.
        let bytes = Zeroizing::new(
            self.channel
                .prepare(frame)
                .map_err(ExchangeError::Protocol)?,
        );
        let result = (|| {
            self.wait(libc::POLLOUT, timeout)?;
            // SAFETY: The descriptor and buffer remain valid throughout the call.
            let sent = unsafe {
                libc::send(
                    self.fd.as_raw_fd(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                )
            };
            if sent < 0 {
                return Err(ExchangeError::Io(std::io::Error::last_os_error()));
            }
            if sent as usize != bytes.len() {
                return Err(ExchangeError::Io(std::io::Error::other(
                    "неполная отправка пакета",
                )));
            }
            Ok(())
        })();
        if result.is_err() {
            self.fail();
        }
        result
    }

    /// Accepts one frame; the worker is not allowed to transmit the descriptors here.
    pub fn receive(&mut self, timeout: Duration) -> Result<ProviderFrame, ExchangeError> {
        if self.failed {
            return Err(ExchangeError::Closed);
        }
        let result = (|| {
            self.wait(libc::POLLIN, timeout)?;
            let mut bytes = Zeroizing::new(vec![0u8; carosign_protocol::limits::FRAME_MAX]);
            let mut iov = libc::iovec {
                iov_base: bytes.as_mut_ptr().cast(),
                iov_len: bytes.len(),
            };
            // Without a control buffer, the kernel closes the sent SCM_RIGHTS note
            // MSG_CTRUNC, which we reject.
            // SAFETY: Zero msghdr, let’s say iov and buffer work.
            let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
            message.msg_iov = &mut iov;
            message.msg_iovlen = 1;
            // SAFETY: The structure and buffer remain valid throughout `recvmsg`.
            let count = unsafe {
                libc::recvmsg(
                    self.fd.as_raw_fd(),
                    &mut message,
                    libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
                )
            };
            if count < 0 {
                return Err(ExchangeError::Io(std::io::Error::last_os_error()));
            }
            if message.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
                return Err(ExchangeError::Truncated);
            }
            if count == 0 {
                return Err(ExchangeError::Closed);
            }
            bytes.truncate(count as usize);
            self.channel.accept(&bytes).map_err(ExchangeError::Protocol)
        })();
        if result.is_err() {
            self.fail();
        }
        result
    }

    /// Transfers sole ownership further, keeping personnel numbers.
    /// The damaged channel cannot be returned.
    pub fn into_parts(self) -> Result<(OwnedFd, Channel), ExchangeError> {
        if self.failed {
            return Err(ExchangeError::Closed);
        }
        Ok((self.fd, self.channel))
    }

    fn fail(&mut self) {
        self.failed = true;
        // SAFETY: The socket belongs to the object; a disabling ends the exchange even if the
        // owner has not yet released the object after the error.
        unsafe { libc::shutdown(self.fd.as_raw_fd(), libc::SHUT_RDWR) };
    }

    /// Interrupts the session after violating protocol.
    pub fn abort(&mut self) {
        self.fail();
    }

    fn wait(&self, events: libc::c_short, timeout: Duration) -> Result<(), ExchangeError> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ExchangeError::Timeout)?;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ExchangeError::Timeout);
            }
            let millis = remaining
                .as_millis()
                .saturating_add(1)
                .min(i32::MAX as u128) as i32;
            let mut pollfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events,
                revents: 0,
            };
            // SAFETY: pollfd indicates one valid element.
            let result = unsafe { libc::poll(&mut pollfd, 1, millis) };
            if result > 0 {
                return Ok(());
            }
            if result == 0 {
                continue;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(ExchangeError::Io(error));
            }
        }
    }
}

// The form of unnamed addresses does not prove the start of a proven process: the owner of
// the closed pair and the verified spawn remain authoritative.
pub(crate) fn unnamed_unix_address(fd: libc::c_int, peer: bool) -> bool {
    // SAFETY: The null structure and its size are acceptable for getockname.
    let mut address: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of_val(&address) as libc::socklen_t;
    // SAFETY: The buffer has room for `sockaddr_storage`. The kernel updates only the buffer and its
    // length.
    let result = unsafe {
        if peer {
            libc::getpeername(
                fd,
                (&mut address as *mut libc::sockaddr_storage).cast(),
                &mut length,
            )
        } else {
            libc::getsockname(
                fd,
                (&mut address as *mut libc::sockaddr_storage).cast(),
                &mut length,
            )
        }
    };
    result == 0
        && address.ss_family as libc::c_int == libc::AF_UNIX
        && length as usize == std::mem::size_of::<libc::sa_family_t>()
}

#[path = "provider_transport.tests.rs"]
#[cfg(test)]
mod tests;
