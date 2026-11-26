use super::{Connection, ConversationEnd, FRAME_BUFFER};

pub(super) fn check_frame_size(frame: &[u8]) -> Result<(), ConversationEnd> {
    if frame.len() > FRAME_BUFFER {
        return Err(ConversationEnd::Refused("кадр больше предела".to_string()));
    }
    Ok(())
}

/// Reads exactly one message. `SOCK_SEQPACKET`.
///
/// ⚠️ Returns. `Ok(None)` It's closing, not making a mistake: closing is the end of the
/// conversation, not the problem. The interruption is read out: the signal does not cancel
/// the frame.
pub fn read_frame(connection: &Connection, buffer: &mut [u8]) -> Result<Option<usize>, i32> {
    use std::os::fd::AsRawFd;
    loop {
        let mut iov = libc::iovec {
            iov_base: buffer.as_mut_ptr().cast(),
            iov_len: buffer.len(),
        };
        // The public channel does not accept ancillary data. Without a control buffer, the
        // kernel closes the transmitted data. SCM_RIGHTS put out MSG_CTRUNC.
        // SAFETY: The zero state of msghdr is permissible, iov lives to recvmsg.
        let mut msg: libc::msghdr = unsafe { core::mem::zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        // SAFETY: The descriptor and buffer addresses are valid at the time of the call.
        let rc =
            unsafe { libc::recvmsg(connection.fd.as_raw_fd(), &mut msg, libc::MSG_CMSG_CLOEXEC) };
        if rc >= 0 && msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
            return Err(libc::EMSGSIZE);
        }
        if rc > 0 {
            return Ok(Some(rc as usize));
        }
        if rc == 0 {
            return Ok(None);
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(error.raw_os_error().unwrap_or(-1));
    }
}

/// Sends one whole shot.
///
/// ⚠️ `MSG_NOSIGNAL`The cliff at that end is no reason to kill the service with a signal.
/// The record does not wait for the client: the filled buffer returns `EAGAIN` to the owner.
pub fn write_frame(connection: &Connection, bytes: &[u8]) -> Result<(), i32> {
    use std::os::fd::AsRawFd;
    if bytes.is_empty() || bytes.len() > FRAME_BUFFER {
        return Err(libc::EMSGSIZE);
    }
    loop {
        // SAFETY: The descriptor is valid, and the kernel receives the owned buffer and its length.
        let rc = unsafe {
            libc::send(
                connection.fd.as_raw_fd(),
                bytes.as_ptr() as *const core::ffi::c_void,
                bytes.len(),
                libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT,
            )
        };
        if rc < 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
            if errno == libc::EINTR {
                continue;
            }
            return Err(errno);
        }
        return if rc as usize == bytes.len() {
            Ok(())
        } else {
            Err(libc::EIO)
        };
    }
}
