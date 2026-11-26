use std::io;
use std::os::fd::{AsRawFd, OwnedFd};

/// Peer-to-peer credits given by a trusted channel owner.
#[derive(Clone, Copy)]
pub struct ExpectedPeer {
    pub pid: u32,
    pub uid: u32,
}

pub(super) fn check_peer(fd: &OwnedFd, peer: ExpectedPeer) -> io::Result<()> {
    let mut kind: libc::c_int = 0;
    let mut len = std::mem::size_of_val(&kind) as libc::socklen_t;
    // SAFETY: Both variables are valid until getockopt returns.
    if unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut libc::c_int).cast(),
            &mut len,
        )
    } != 0
        || kind != libc::SOCK_SEQPACKET
        || !crate::provider::transport::unnamed_unix_address(fd.as_raw_fd(), false)
        || !crate::provider::transport::unnamed_unix_address(fd.as_raw_fd(), true)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "неверный тип канала",
        ));
    }
    let mut credential = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of_val(&credential) as libc::socklen_t;
    // SAFETY: The ucred buffer and its length are correct.
    if unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credential as *mut libc::ucred).cast(),
            &mut len,
        )
    } != 0
        || len as usize != std::mem::size_of::<libc::ucred>()
        || peer.pid == 0
        || credential.pid <= 0
        || credential.pid as u32 != peer.pid
        || credential.uid != peer.uid
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "неверный владелец канала",
        ));
    }
    // SAFETY: fd belongs to the object; CLOEXEC does not alter the rights of the socket.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
    if flags < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
