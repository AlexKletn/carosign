use std::time::{Duration, Instant};

use crate::transport::{PacketSocket, TransportError};

pub(super) fn wait_packet(
    socket: &PacketSocket,
    deadline: Instant,
) -> Result<Vec<u8>, TransportError> {
    loop {
        match socket.receive() {
            Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1))
            }
            other => return other,
        }
    }
}

pub(super) fn send_packet(
    socket: &PacketSocket,
    bytes: &[u8],
    deadline: Instant,
) -> Result<(), TransportError> {
    loop {
        match socket.send(bytes) {
            Err(TransportError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1))
            }
            other => return other,
        }
    }
}
