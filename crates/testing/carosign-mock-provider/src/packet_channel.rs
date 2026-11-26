use std::os::fd::{FromRawFd, OwnedFd, RawFd};

pub(super) fn validate_channel(fd: RawFd) -> Result<(), String> {
    let mut kind: libc::c_int = 0;
    let mut length = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: Type and length indicate correct buffers before a call is returned.
    let result = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut libc::c_int).cast(),
            &mut length,
        )
    };
    if result != 0 || kind != libc::SOCK_SEQPACKET {
        return Err("channel is not seqpacket".into());
    }
    // SAFETY: zero-point sockaddr_storage Suitable as a kernel address buffer.
    let mut address: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut address_len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    // SAFETY: The address and its length indicate the allocated buffer.
    let result = unsafe {
        libc::getpeername(
            fd,
            (&mut address as *mut libc::sockaddr_storage).cast(),
            &mut address_len,
        )
    };
    if result != 0 || address.ss_family as libc::c_int != libc::AF_UNIX {
        return Err("channel is not connected unix socket".into());
    }
    Ok(())
}

pub(super) fn send(fd: RawFd, bytes: &[u8]) -> Result<(), String> {
    // SAFETY: The descriptor and buffer remain valid throughout the system call.
    let size = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL) };
    if size != bytes.len() as isize {
        return Err("send failed".into());
    }
    Ok(())
}

pub(super) type ReceivedPacket = (Vec<u8>, Vec<OwnedFd>);

pub(super) fn receive(fd: RawFd) -> Result<Option<ReceivedPacket>, String> {
    let mut bytes = vec![0; carosign_protocol::limits::FRAME_MAX];
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: A zero msghdr and one valid iovec are valid for recvmsg.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    let mut control = [0usize; 32];
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = std::mem::size_of_val(&control);
    // SAFETY: The buffer and title live until the call is returned.
    let size = unsafe { libc::recvmsg(fd, &mut msg, libc::MSG_CMSG_CLOEXEC) };
    if size < 0 {
        return Err("receive failed".into());
    }
    if msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 {
        return Err("truncated or unexpected descriptors".into());
    }
    if size == 0 {
        return Ok(None);
    }
    bytes.truncate(size as usize);
    let mut descriptors = Vec::new();
    let header = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    if !header.is_null() {
        if unsafe {
            (*header).cmsg_level != libc::SOL_SOCKET
                || (*header).cmsg_type != libc::SCM_RIGHTS
                || (*header).cmsg_len
                    != libc::CMSG_LEN(std::mem::size_of::<RawFd>() as u32) as usize
                || !libc::CMSG_NXTHDR(&msg, header).is_null()
        } {
            return Err("invalid descriptor control".into());
        }
        let received = unsafe { *libc::CMSG_DATA(header).cast::<RawFd>() };
        descriptors.push(unsafe { OwnedFd::from_raw_fd(received) });
    }
    Ok(Some((bytes, descriptors)))
}
