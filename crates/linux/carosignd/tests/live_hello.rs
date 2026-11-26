//! Live conversation: a real socket, a real customer, a real welcome.
//!
//! Exercises kernel-supplied `SO_PEERCRED`, `accept4`, and delivery over a real socket.

use carosign_linux::sys;
use carosign_protocol::carosign::client::v1::{self as client, ClientFrame, client_frame::Payload};
use carosignd::server::{Connection, ConversationEnd, accept_one, greet, new_connection};
use prost::Message;

fn temp_socket(name: &str) -> String {
    let mut path = std::env::temp_dir();
    path.push(format!("carosignd-{name}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path.to_str().expect("путь").to_string()
}

#[test]
fn real_socket_accepts_and_greets() {
    let path = temp_socket("hello");
    let listener = sys::listen_unix(&path).expect("слушатель");

    // The test uses a `SOCK_SEQPACKET` client. `UnixStream::connect` opens a `SOCK_STREAM`
    // socket and the kernel replies with `EPROTOTYPE`.
    let client_path = path.clone();
    let client = std::thread::spawn(move || {
        use std::io::Read;
        let mut stream = seqpacket_connect(&client_path).expect("подключение");
        let hello = ClientFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            request_id: vec![1u8; 16],
            operation_id: Vec::new(),
            event_sequence: 0,
            payload: Some(Payload::Hello(client::Hello {
                capture_feedback_v1: false,
                major: 1,
                min_minor: carosign_protocol::limits::MINOR_BASELINE,
                max_minor: carosign_protocol::limits::MINOR_MAX,
            })),
        }
        .encode_to_vec();
        send_frame(&stream, &hello).expect("отправлено");
        let mut answer = vec![0u8; 4096];
        let read = stream.read(&mut answer).expect("ответ");
        answer.truncate(read);
        answer
    });

    let connection_id = new_connection().unwrap();
    let mut connection: Connection = accept_one(&listener, connection_id).expect("принято");
    // ⚠️ Creeds came from the core: uid and pid are the same as ours.
    // SAFETY: geteuid does not accept pointers and does not change the state.
    assert_eq!(connection.peer.uid, unsafe { libc::geteuid() });

    let mut frame = vec![0u8; 4096];
    let received = read_frame(&connection, &mut frame);
    let answer = greet(&mut connection, &frame[..received]).expect("приветствие");
    assert!(!answer.is_empty());
    // Send a response, otherwise the client waits until the end of the century.
    {
        use std::os::fd::AsRawFd;
        // SAFETY: The descriptor is valid, the buffer is ours.
        let rc = unsafe {
            libc::send(
                connection.fd.as_raw_fd(),
                answer.as_ptr() as *const core::ffi::c_void,
                answer.len(),
                0,
            )
        };
        assert!(rc > 0, "ответ не отправлен");
    }

    let reply = client.join().expect("клиент");
    let parsed = ClientFrame::decode(&reply[..]).expect("ответ разбирается");
    match parsed.payload {
        Some(Payload::HelloAck(ack)) => {
            assert_eq!(ack.selected_minor, carosign_protocol::limits::MINOR_MAX);
            assert_eq!(ack.connection_id, connection_id.to_vec());
        }
        other => panic!("ожидался HelloAck, а пришло {other:?}"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_hello_is_refused_on_a_real_channel() {
    let path = temp_socket("refuse");
    let listener = sys::listen_unix(&path).expect("слушатель");

    let client_path = path.clone();
    let client = std::thread::spawn(move || {
        let stream = seqpacket_connect(&client_path).expect("подключение");
        let health = ClientFrame {
            major: 1,
            minor: carosign_protocol::limits::MINOR_MAX,
            request_id: vec![2u8; 16],
            operation_id: Vec::new(),
            event_sequence: 0,
            payload: Some(Payload::GetHealth(client::GetHealth {})),
        }
        .encode_to_vec();
        send_frame(&stream, &health).expect("отправлено");
        // ⚠️ We do not close immediately: the service can read only after `accept`Let her
        // accept, it's not waiting for an answer, it's a pause in sending.
        std::thread::sleep(std::time::Duration::from_millis(200));
    });

    let mut connection = accept_one(&listener, new_connection().unwrap()).expect("принято");
    let mut frame = vec![0u8; 4096];
    let received = read_frame(&connection, &mut frame);
    assert!(matches!(
        greet(&mut connection, &frame[..received]),
        Err(ConversationEnd::Refused(_))
    ));
    client.join().expect("клиент");
    let _ = std::fs::remove_file(&path);
}

/// Reads one frame without waiting for the connection to close.
///
/// ⚠️ `SOCK_SEQPACKET` Gives exactly one message; `Read` If you had a copy of the descriptor,
/// you'd have EOF waiting, and you wouldn't have it.
fn read_frame(connection: &Connection, buffer: &mut [u8]) -> usize {
    use std::os::fd::AsRawFd;
    // SAFETY: The descriptor is valid, the buffer is ours.
    let rc = unsafe {
        libc::recv(
            connection.fd.as_raw_fd(),
            buffer.as_mut_ptr() as *mut core::ffi::c_void,
            buffer.len(),
            0,
        )
    };
    assert!(
        rc > 0,
        "кадр не прочитан: errno {}",
        std::io::Error::last_os_error()
    );
    rc as usize
}

/// Client's `SOCK_SEQPACKET` s `SOCK_CLOEXEC`.
fn seqpacket_connect(path: &str) -> std::io::Result<std::os::unix::net::UnixStream> {
    use std::os::fd::FromRawFd;
    let c_path = std::ffi::CString::new(path).expect("путь");
    // SAFETY: regular calls; the result is checked.
    unsafe {
        let fd = libc::socket(libc::AF_UNIX, libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC, 0);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut address: libc::sockaddr_un = core::mem::zeroed();
        address.sun_family = libc::AF_UNIX as libc::sa_family_t;
        let bytes = path.as_bytes();
        for (index, byte) in bytes.iter().enumerate() {
            address.sun_path[index] = *byte as libc::c_char;
        }
        let length =
            (core::mem::size_of::<libc::sa_family_t>() + bytes.len() + 1) as libc::socklen_t;
        if libc::connect(
            fd,
            &address as *const libc::sockaddr_un as *const libc::sockaddr,
            length,
        ) != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let _ = c_path;
        Ok(std::os::unix::net::UnixStream::from_raw_fd(fd))
    }
}

/// One whole message.
fn send_frame(stream: &std::os::unix::net::UnixStream, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut stream = stream.try_clone()?;
    stream.write_all(bytes)
}
