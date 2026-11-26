use super::*;
use carosign_linux::sys::socketpair_seqpacket;
use carosign_protocol::carosign::provider::v1::{BindAuthenticator, FdSlot};
use std::os::fd::{AsFd, FromRawFd};

#[test]
fn packets_are_preserved_and_truncation_is_rejected() {
    let pair = socketpair_seqpacket().unwrap();
    let a = PacketSocket::new(pair.platform).unwrap();
    let b = PacketSocket::new(pair.worker).unwrap();
    assert!(matches!(b.receive(), Err(TransportError::WouldBlock)));
    a.send(b"first").unwrap();
    a.send(b"second").unwrap();
    assert_eq!(b.receive().unwrap(), b"first");
    assert_eq!(b.receive().unwrap(), b"second");
    assert!(matches!(
        a.send(&vec![0; FRAME_MAX + 1]),
        Err(TransportError::Invalid)
    ));
    // A sender outside our process may violate the restriction.
    let oversize = vec![1; FRAME_MAX + 1];
    let n = unsafe {
        libc::send(
            a.0.as_raw_fd(),
            oversize.as_ptr().cast(),
            oversize.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    assert_eq!(n as usize, oversize.len());
    assert!(matches!(b.receive(), Err(TransportError::Invalid)));
}

#[test]
fn tunnel_rejects_descriptors_and_bind_checks_slots() {
    let pair = socketpair_seqpacket().unwrap();
    let device = socketpair_seqpacket().unwrap();
    carosign_linux::sys::send_fd(&pair.platform, &device.platform).unwrap();
    let b = PacketSocket::new(pair.worker).unwrap();
    assert!(matches!(b.receive(), Err(TransportError::Invalid)));
    assert!(matches!(b.receive(), Err(TransportError::Closed)));
    let pair = socketpair_seqpacket().unwrap();
    let b = PacketSocket::new(pair.worker).unwrap();
    let a = PacketSocket::new(pair.platform).unwrap();
    let mut frame = ProviderFrame {
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            fd_slots: vec![FdSlot {
                index: 0,
                role: FdRole::Device as i32,
                resource_id: vec![1; 16],
            }],
            ..Default::default()
        })),
        ..Default::default()
    };
    assert!(matches!(
        a.send_bind(&frame, &[]),
        Err(TransportError::Invalid)
    ));
    if let Some(Payload::BindAuthenticator(bind)) = &mut frame.payload {
        bind.fd_slots[0].index = 1;
    }
    assert!(matches!(
        a.send_bind(&frame, &[device.platform.as_fd()]),
        Err(TransportError::Invalid)
    ));
    if let Some(Payload::BindAuthenticator(bind)) = &mut frame.payload {
        bind.fd_slots[0].index = 0;
    }
    a.send_bind(&frame, &[device.platform.as_fd()]).unwrap();
    // The receiver without permission for FD must close the channel without saving FD.
    assert!(matches!(b.receive(), Err(TransportError::Invalid)));
}

#[test]
fn bind_is_one_packet_with_working_descriptor() {
    const ENV: &str = "CAROSIGN_BROKER_BIND_TEST_FD";
    if let Ok(value) = std::env::var(ENV) {
        // SAFETY: The socket descriptor was passed on by the test parent.
        let socket = unsafe { OwnedFd::from_raw_fd(value.parse().unwrap()) };
        receive_bind_and_write_device(&socket);
        return;
    }
    let pair = socketpair_seqpacket().unwrap();
    let device = socketpair_seqpacket().unwrap();
    let a = PacketSocket::new(pair.platform).unwrap();
    let frame = ProviderFrame {
        payload: Some(Payload::BindAuthenticator(BindAuthenticator {
            fd_slots: vec![FdSlot {
                index: 0,
                role: FdRole::Device as i32,
                resource_id: vec![1; 16],
            }],
            ..Default::default()
        })),
        ..Default::default()
    };
    let receiver = pair.worker.as_raw_fd();
    // SAFETY: The flag changes at the test descriptor.
    let flags = unsafe { libc::fcntl(receiver, libc::F_GETFD) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(receiver, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
        0
    );
    a.send_bind(&frame, &[device.platform.as_fd()]).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "transport::packet_socket::tests::bind_is_one_packet_with_working_descriptor",
            "--nocapture",
        ])
        .env(ENV, receiver.to_string())
        .spawn()
        .unwrap();
    drop(pair.worker);
    assert!(child.wait().unwrap().success());
    let mut result = [0u8; 16];
    let n = unsafe {
        libc::recv(
            device.worker.as_raw_fd(),
            result.as_mut_ptr().cast(),
            result.len(),
            0,
        )
    };
    assert_eq!(&result[..n as usize], b"device");
}

fn receive_bind_and_write_device(socket: &OwnedFd) {
    let mut buf = [0u8; 1024];
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    let space = unsafe { libc::CMSG_SPACE(size_of::<libc::c_int>() as u32) as usize };
    let mut control = vec![0usize; space.div_ceil(size_of::<usize>())];
    let mut msg: libc::msghdr = unsafe { zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = space;
    let count = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, libc::MSG_CMSG_CLOEXEC) };
    assert!(count > 0);
    let frame = ProviderFrame::decode(&buf[..count as usize]).unwrap();
    assert!(
        matches!(frame.payload, Some(Payload::BindAuthenticator(ref bind)) if bind.fd_slots.len() == 1)
    );
    assert_eq!(msg.msg_flags & (libc::MSG_CTRUNC | libc::MSG_TRUNC), 0);
    let header = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    assert!(!header.is_null());
    assert_eq!(unsafe { (*header).cmsg_level }, libc::SOL_SOCKET);
    assert_eq!(unsafe { (*header).cmsg_type }, libc::SCM_RIGHTS);
    assert_eq!(unsafe { (*header).cmsg_len }, unsafe {
        libc::CMSG_LEN(size_of::<libc::c_int>() as u32) as usize
    });
    // SAFETY: The kernel returned one new descriptor via `SCM_RIGHTS`.
    let received = unsafe { OwnedFd::from_raw_fd(*libc::CMSG_DATA(header).cast::<libc::c_int>()) };
    let value = b"device";
    assert_eq!(
        unsafe {
            libc::send(
                received.as_raw_fd(),
                value.as_ptr().cast(),
                value.len(),
                libc::MSG_NOSIGNAL,
            )
        },
        value.len() as isize
    );
}

#[test]
fn separate_worker_exchange() {
    const ENV: &str = "CAROSIGN_BROKER_TEST_WORKER_FD";
    if let Ok(value) = std::env::var(ENV) {
        let raw: i32 = value.parse().unwrap();
        // SAFETY: The descriptor is passed on by the test parent only to this process.
        let socket = PacketSocket::new(unsafe { OwnedFd::from_raw_fd(raw) }).unwrap();
        loop {
            match socket.receive() {
                Ok(value) => {
                    assert_eq!(value, b"command");
                    break;
                }
                Err(TransportError::WouldBlock) => std::thread::yield_now(),
                Err(error) => panic!("ошибка: {error:?}"),
            }
        }
        socket.send(b"event").unwrap();
        return;
    }
    let pair = socketpair_seqpacket().unwrap();
    let fd = pair.worker.as_raw_fd();
    // SAFETY: fd is in the possession of the test, the flag is restored before the exit.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) },
        0
    );
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "transport::packet_socket::tests::separate_worker_exchange",
            "--nocapture",
        ])
        .env(ENV, fd.to_string())
        .spawn()
        .unwrap();
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_SETFD, flags) }, 0);
    drop(pair.worker);
    let socket = PacketSocket::new(pair.platform).unwrap();
    socket.send(b"command").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match socket.receive() {
            Ok(value) => {
                assert_eq!(value, b"event");
                break;
            }
            Err(TransportError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::yield_now()
            }
            Err(error) => panic!("ошибка: {error:?}"),
        }
    }
    assert!(child.wait().unwrap().success());
}
