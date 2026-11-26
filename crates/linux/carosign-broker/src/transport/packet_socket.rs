use super::TransportError;

use std::cell::Cell;
use std::io;
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};

use carosign_protocol::carosign::provider::v1::{FdRole, ProviderFrame, provider_frame::Payload};
use carosign_protocol::limits::FRAME_MAX;
use prost::Message;

fn io_error() -> TransportError {
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::WouldBlock {
        TransportError::WouldBlock
    } else {
        TransportError::Io(error)
    }
}

/// The sole owner of the already created combined socket SEQPACKET.
pub struct PacketSocket(OwnedFd, Cell<bool>);

impl PacketSocket {
    /// Checks the type of socket, checks the origin of the canal stays outside.
    pub fn new(fd: OwnedFd) -> Result<Self, TransportError> {
        Self::from_unix(fd, false)
    }

    pub(crate) fn from_discovered_worker(fd: OwnedFd) -> Result<Self, TransportError> {
        Self::from_unix(fd, true)
    }

    fn from_unix(fd: OwnedFd, discovered_worker: bool) -> Result<Self, TransportError> {
        let mut kind: libc::c_int = 0;
        let mut len = size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: pointers refer to actual variables of the desired size.
        let rc = unsafe {
            libc::getsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut libc::c_int).cast(),
                &mut len,
            )
        };
        if rc != 0 {
            return Err(io_error());
        }
        if len as usize != size_of::<libc::c_int>()
            || kind != libc::SOCK_SEQPACKET
            || !unix_address(fd.as_raw_fd(), false, !discovered_worker)
            || !unnamed_unix(fd.as_raw_fd(), true)
        {
            return Err(TransportError::Invalid);
        }
        // SAFETY: The descriptor belongs to this object.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
        {
            return Err(io_error());
        }
        Ok(Self(fd, Cell::new(false)))
    }

    fn fail(&self) {
        self.1.set(true);
        // SAFETY: The object owns the socket until the completion of this subroutine.
        unsafe { libc::shutdown(self.0.as_raw_fd(), libc::SHUT_RDWR) };
    }

    pub(crate) fn close(&self) {
        self.fail();
    }

    /// It gets one packet without the descriptors. The extra ancillary closes the kernel.
    /// Any waiver except `WouldBlock` requires the connection to be closed by the caller.
    pub fn receive(&self) -> Result<Vec<u8>, TransportError> {
        if self.1.get() {
            return Err(TransportError::Closed);
        }
        let mut data = vec![0u8; FRAME_MAX];
        let mut iov = libc::iovec {
            iov_base: data.as_mut_ptr().cast(),
            iov_len: data.len(),
        };
        // Without a control buffer, the kernel does not install the received SCM_RIGHTS;
        // MSG_CTRUNC It shows that the forbidden ancillary has come.
        let mut msg: libc::msghdr = unsafe { zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        // SAFETY: The buffer and title are valid for the time of the call.
        let received = unsafe {
            libc::recvmsg(
                self.0.as_raw_fd(),
                &mut msg,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if received < 0 {
            let error = io_error();
            if !matches!(error, TransportError::WouldBlock) {
                self.fail();
            }
            return Err(error);
        }
        if received == 0 {
            self.fail();
            return Err(TransportError::Closed);
        }
        if msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
            self.fail();
            return Err(TransportError::Invalid);
        }
        data.truncate(received as usize);
        Ok(data)
    }

    /// It sends exactly one packet without ancillary.
    pub fn send(&self, data: &[u8]) -> Result<(), TransportError> {
        self.send_packet(data, &[])
    }

    /// Transmits provider Bind and associated descriptors with a single system call.
    /// Verifications of the role, agreement with issuance and origin of FDs lie above.
    pub fn send_bind(
        &self,
        frame: &ProviderFrame,
        fds: &[BorrowedFd<'_>],
    ) -> Result<(), TransportError> {
        let Some(Payload::BindAuthenticator(bind)) = &frame.payload else {
            return Err(TransportError::Invalid);
        };
        if fds.len() > 8 || bind.fd_slots.len() != fds.len() {
            return Err(TransportError::Invalid);
        }
        for (index, slot) in bind.fd_slots.iter().enumerate() {
            if slot.index as usize != index
                || slot.resource_id.len() != 16
                || !slot.resource_id.iter().any(|b| *b != 0)
                || bind.fd_slots[..index]
                    .iter()
                    .any(|previous| previous.resource_id == slot.resource_id)
                || !matches!(
                    FdRole::from_i32(slot.role),
                    Some(FdRole::Device | FdRole::Event | FdRole::Config | FdRole::MediatedService)
                )
            {
                return Err(TransportError::Invalid);
            }
        }
        self.send_packet(&frame.encode_to_vec(), fds)
    }

    fn send_packet(&self, data: &[u8], fds: &[BorrowedFd<'_>]) -> Result<(), TransportError> {
        if self.1.get() {
            return Err(TransportError::Closed);
        }
        if data.is_empty() || data.len() > FRAME_MAX || fds.len() > 8 {
            return Err(TransportError::Invalid);
        }
        let mut iov = libc::iovec {
            iov_base: data.as_ptr().cast_mut().cast(),
            iov_len: data.len(),
        };
        let mut msg: libc::msghdr = unsafe { zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        // The usize alignment corresponds to cmsghdr, the place is reserved until sendmsg.
        let space = if fds.is_empty() {
            0
        } else {
            unsafe { libc::CMSG_SPACE((fds.len() * size_of::<libc::c_int>()) as u32) as usize }
        };
        let mut control = vec![0usize; space.div_ceil(size_of::<usize>())];
        if !fds.is_empty() {
            msg.msg_control = control.as_mut_ptr().cast();
            msg.msg_controllen = space;
            // SAFETY: The control buffer is aligned and sized with `CMSG_SPACE` for all descriptors.
            unsafe {
                let cmsg = libc::CMSG_FIRSTHDR(&msg);
                if cmsg.is_null() {
                    return Err(TransportError::Invalid);
                }
                (*cmsg).cmsg_level = libc::SOL_SOCKET;
                (*cmsg).cmsg_type = libc::SCM_RIGHTS;
                (*cmsg).cmsg_len =
                    libc::CMSG_LEN((fds.len() * size_of::<libc::c_int>()) as u32) as usize;
                let target = libc::CMSG_DATA(cmsg).cast::<libc::c_int>();
                for (index, fd) in fds.iter().enumerate() {
                    *target.add(index) = fd.as_raw_fd();
                }
            }
        }
        // SAFETY: All pointers refer to data that lives until syscall returns.
        let written = unsafe {
            libc::sendmsg(
                self.0.as_raw_fd(),
                &msg,
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if written < 0 {
            let error = io_error();
            if !matches!(error, TransportError::WouldBlock) {
                self.fail();
            }
            return Err(error);
        }
        if written as usize != data.len() {
            self.fail();
            return Err(TransportError::Invalid);
        }
        Ok(())
    }
}

pub(crate) fn unnamed_unix(fd: libc::c_int, peer: bool) -> bool {
    unix_address(fd, peer, true)
}

fn unix_address(fd: libc::c_int, peer: bool, unnamed: bool) -> bool {
    // SAFETY: The structure and length are valid for calling the core.
    let mut address: libc::sockaddr_storage = unsafe { zeroed() };
    let mut length = size_of::<libc::sockaddr_storage>() as libc::socklen_t;
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
        && if unnamed {
            length as usize == size_of::<libc::sa_family_t>()
        } else {
            length as usize > size_of::<libc::sa_family_t>()
        }
}

#[path = "packet_socket.tests.rs"]
#[cfg(test)]
mod tests;
