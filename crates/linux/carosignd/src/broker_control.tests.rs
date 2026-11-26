use super::*;
use std::os::fd::FromRawFd;

fn pair() -> (OwnedFd, OwnedFd) {
    let mut raw = [0; 2];
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                raw.as_mut_ptr(),
            )
        },
        0
    );
    // SAFETY: Socketpair passed two different descriptors to this test.
    unsafe { (OwnedFd::from_raw_fd(raw[0]), OwnedFd::from_raw_fd(raw[1])) }
}

#[test]
fn separate_control_completes_while_operation_receive_is_blocked() {
    let (operation, idle) = pair();
    let (control, peer) = pair();
    let blocked = std::thread::spawn(move || {
        let mut byte = 0u8;
        // SAFETY: The buffer and socket belong to the stream.
        unsafe { libc::recv(operation.as_raw_fd(), (&mut byte as *mut u8).cast(), 1, 0) }
    });
    let server = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let n = unsafe { libc::recv(peer.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
        let request = Control::decode(&buf[..n as usize]).unwrap();
        let response = ControlResult {
            request_id: request.request_id,
            result: Some(control_result::Result::ExclusiveQuiesced(true)),
        }
        .encode_to_vec();
        assert_eq!(
            unsafe {
                libc::send(
                    peer.as_raw_fd(),
                    response.as_ptr().cast(),
                    response.len(),
                    0,
                )
            },
            response.len() as isize
        );
    });
    assert!(revoke(control, Scope::default(), [1; 16], Duration::from_secs(1)).is_ok());
    drop(idle);
    assert_eq!(blocked.join().unwrap(), 0);
    server.join().unwrap();
}

#[test]
fn lost_response_times_out_without_confirming_revoke() {
    let (control, _peer) = pair();
    assert_eq!(
        revoke(
            control,
            Scope::default(),
            [1; 16],
            Duration::from_millis(20)
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::TimedOut
    );
}

#[test]
fn revoked_descriptors_do_not_prove_operation_completion() {
    let (control, peer) = pair();
    let server = std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        let n = unsafe { libc::recv(peer.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
        let request = Control::decode(&buf[..n as usize]).unwrap();
        let response = ControlResult {
            request_id: request.request_id,
            result: Some(control_result::Result::ExclusiveRevoked(true)),
        }
        .encode_to_vec();
        assert_eq!(
            unsafe {
                libc::send(
                    peer.as_raw_fd(),
                    response.as_ptr().cast(),
                    response.len(),
                    0,
                )
            },
            response.len() as isize
        );
    });
    assert_eq!(
        revoke(control, Scope::default(), [1; 16], Duration::from_secs(1))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    server.join().unwrap();
}
