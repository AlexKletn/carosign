use super::*;

#[test]
fn pending_packet_and_disconnect_are_visible_before_commit() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let probe = CancelReader {
        _target: None,
        stop: Arc::new(AtomicBool::new(false)),
        handling: Arc::new(AtomicBool::new(false)),
        probe: pair.platform,
        thread: None,
    };
    assert!(!probe.pending_cancel_or_closed());
    let packet = b"request";
    assert_eq!(
        unsafe {
            libc::send(
                pair.worker.as_raw_fd(),
                packet.as_ptr().cast(),
                packet.len(),
                libc::MSG_NOSIGNAL,
            )
        },
        packet.len() as isize
    );
    assert!(probe.pending_cancel_or_closed());
    // The check does not take the package away from the only reader.
    let mut buffer = [0u8; 16];
    assert_eq!(
        unsafe {
            libc::recv(
                probe.probe.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                libc::MSG_DONTWAIT,
            )
        },
        packet.len() as isize
    );
    assert!(!probe.pending_cancel_or_closed());
    drop(pair.worker);
    assert!(probe.pending_cancel_or_closed());
}

#[test]
fn reader_processing_flag_closes_consumption_race() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let probe = CancelReader {
        _target: None,
        stop: Arc::new(AtomicBool::new(false)),
        handling: Arc::new(AtomicBool::new(true)),
        probe: pair.platform,
        thread: None,
    };
    assert!(probe.pending_cancel_or_closed());
}
