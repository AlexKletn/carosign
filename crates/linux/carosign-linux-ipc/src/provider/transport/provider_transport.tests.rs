use super::*;
use std::os::fd::FromRawFd;

fn pair(kind: libc::c_int) -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    // SAFETY: The array has room for two descriptors.
    assert_eq!(
        unsafe { libc::socketpair(libc::AF_UNIX, kind, 0, fds.as_mut_ptr()) },
        0
    );
    // SAFETY: A successful socketpair gave us ownership of both descriptors.
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

fn hello() -> ProviderFrame {
    ProviderFrame {
        major: 1,
        payload: Some(Payload::WorkerHello(
            carosign_protocol::carosign::provider::v1::WorkerHello {
                major: 1,
                implementation_version: "probe".into(),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

#[test]
fn send_timeout_closes_channel_and_forbids_retry() {
    let (fd, peer) = pair(libc::SOCK_SEQPACKET);
    let mut transport = ProviderTransport::new(fd, Channel::new()).unwrap();
    assert!(matches!(
        transport.send(hello(), Duration::ZERO),
        Err(ExchangeError::Timeout)
    ));
    assert!(matches!(
        transport.send(hello(), Duration::from_secs(1)),
        Err(ExchangeError::Closed)
    ));
    let mut byte = [0u8; 1];
    // SAFETY: active socket and buffer; shutdown closes reception
    assert_eq!(
        unsafe { libc::recv(peer.as_raw_fd(), byte.as_mut_ptr().cast(), 1, 0) },
        0
    );
    assert!(matches!(transport.into_parts(), Err(ExchangeError::Closed)));
}

#[test]
fn peer_disconnect_during_send_closes_channel_and_forbids_retry() {
    let (fd, peer) = pair(libc::SOCK_SEQPACKET);
    let mut transport = ProviderTransport::new(fd, Channel::new()).unwrap();
    drop(peer);
    assert!(transport.send(hello(), Duration::from_secs(1)).is_err());
    assert!(matches!(
        transport.send(hello(), Duration::from_secs(1)),
        Err(ExchangeError::Closed)
    ));
    assert!(matches!(transport.into_parts(), Err(ExchangeError::Closed)));
}

fn named_pair(abstract_name: bool) -> (OwnedFd, OwnedFd) {
    let path = format!(
        "/tmp/carosign-provider-{}-{}.sock",
        std::process::id(),
        if abstract_name { "abstract" } else { "path" }
    );
    let mut name = path.as_bytes().to_vec();
    if abstract_name {
        name[0] = 0;
    } else {
        let _ = std::fs::remove_file(&path);
    }
    // SAFETY: Socket returns new descriptors, negative ones are not used.
    let listener =
        unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(listener >= 0);
    let listener = unsafe { OwnedFd::from_raw_fd(listener) };
    let client =
        unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0) };
    assert!(client >= 0);
    let client = unsafe { OwnedFd::from_raw_fd(client) };
    // SAFETY: sockaddr_un It allows for zero state.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    assert!(name.len() < address.sun_path.len());
    for (to, from) in address.sun_path.iter_mut().zip(&name) {
        *to = *from as libc::c_char;
    }
    let length = (std::mem::offset_of!(libc::sockaddr_un, sun_path)
        + name.len()
        + usize::from(!abstract_name)) as libc::socklen_t;
    // SAFETY: The address is filled, its length does not exceed the size of the structure.
    assert_eq!(
        unsafe {
            libc::bind(
                listener.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                length,
            )
        },
        0
    );
    assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
    assert_eq!(
        unsafe {
            libc::connect(
                client.as_raw_fd(),
                (&address as *const libc::sockaddr_un).cast(),
                length,
            )
        },
        0
    );
    let accepted = unsafe {
        libc::accept4(
            listener.as_raw_fd(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            libc::SOCK_CLOEXEC,
        )
    };
    assert!(accepted >= 0);
    if !abstract_name {
        std::fs::remove_file(path).unwrap();
    }
    // SAFETY: Accept4 passed a new descriptor.
    (client, unsafe { OwnedFd::from_raw_fd(accepted) })
}

#[test]
fn rejects_pathname_and_abstract_channels_from_both_ends() {
    for abstract_name in [false, true] {
        let (client, peer) = named_pair(abstract_name);
        assert!(matches!(
            ProviderTransport::new(client, Channel::new()),
            Err(ExchangeError::InvalidSocket)
        ));
        drop(peer);
        let (client, accepted) = named_pair(abstract_name);
        assert!(matches!(
            ProviderTransport::new(accepted, Channel::new()),
            Err(ExchangeError::InvalidSocket)
        ));
        drop(client);
    }
}

#[test]
fn refuses_stream_and_closes_after_timeout() {
    let (stream, _peer) = pair(libc::SOCK_STREAM);
    assert!(matches!(
        ProviderTransport::new(stream, Channel::new()),
        Err(ExchangeError::InvalidSocket)
    ));
    let (fd, peer) = pair(libc::SOCK_SEQPACKET);
    let mut transport = ProviderTransport::new(fd, Channel::new()).unwrap();
    assert!(matches!(
        transport.receive(Duration::from_millis(5)),
        Err(ExchangeError::Timeout)
    ));
    let mut byte = [0u8; 1];
    // SAFETY: The descriptor and buffer are valid; the shutdown has already been executed.
    assert_eq!(
        unsafe { libc::recv(peer.as_raw_fd(), byte.as_mut_ptr().cast(), byte.len(), 0) },
        0
    );
    assert!(matches!(
        transport.receive(Duration::from_secs(1)),
        Err(ExchangeError::Closed)
    ));
    assert!(matches!(transport.into_parts(), Err(ExchangeError::Closed)));
}

#[test]
fn rejects_truncated_packet_without_decoding() {
    let (fd, peer) = pair(libc::SOCK_SEQPACKET);
    let mut transport = ProviderTransport::new(fd, Channel::new()).unwrap();
    let bytes = vec![1u8; carosign_protocol::limits::FRAME_MAX + 1];
    // SAFETY: The buffer and descriptor are in place at the time of shipment.
    let sent = unsafe {
        libc::send(
            peer.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            libc::MSG_NOSIGNAL,
        )
    };
    assert_eq!(sent as usize, bytes.len());
    assert!(matches!(
        transport.receive(Duration::from_secs(1)),
        Err(ExchangeError::Truncated)
    ));
    assert!(matches!(transport.into_parts(), Err(ExchangeError::Closed)));
}

#[test]
fn rejects_unexpected_descriptor_and_closes_channel() {
    let (fd, peer) = pair(libc::SOCK_SEQPACKET);
    let mut transport = ProviderTransport::new(fd, Channel::new()).unwrap();
    let file: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    carosign_linux::sys::send_fd(&peer, &file).unwrap();
    assert!(matches!(
        transport.receive(Duration::from_secs(1)),
        Err(ExchangeError::Truncated)
    ));
    assert!(matches!(transport.into_parts(), Err(ExchangeError::Closed)));
}
