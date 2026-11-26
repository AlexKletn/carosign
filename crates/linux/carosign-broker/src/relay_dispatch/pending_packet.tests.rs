use super::*;
use crate::transport::{PacketSocket, TransportError};
use carosign_linux::sys::socketpair_seqpacket;

#[test]
fn pending_packet_is_sent_once_and_revocation_prevents_send() {
    let pair = socketpair_seqpacket().unwrap();
    let sender = PacketSocket::new(pair.platform).unwrap();
    let receiver = PacketSocket::new(pair.worker).unwrap();
    let mut denied = PendingPacket::new(b"denied".to_vec()).unwrap();
    denied.revoke().unwrap();
    assert!(matches!(
        denied.attempt(&sender, true),
        Err(TransportError::Invalid)
    ));
    assert!(matches!(
        receiver.receive(),
        Err(TransportError::WouldBlock)
    ));
    let mut sent = PendingPacket::new(b"sent".to_vec()).unwrap();
    sent.attempt(&sender, true).unwrap();
    assert_eq!(sent.state(), SendState::Sent);
    assert_eq!(receiver.receive().unwrap(), b"sent");
    assert!(matches!(
        sent.attempt(&sender, true),
        Err(TransportError::Invalid)
    ));
    assert!(matches!(
        receiver.receive(),
        Err(TransportError::WouldBlock)
    ));
}

#[test]
fn full_socket_keeps_packet_for_recheck_and_revocation() {
    let pair = socketpair_seqpacket().unwrap();
    let sender = PacketSocket::new(pair.platform).unwrap();
    let receiver = PacketSocket::new(pair.worker).unwrap();
    let filler = vec![7; 4096];
    loop {
        match sender.send(&filler) {
            Ok(()) => (),
            Err(TransportError::WouldBlock) => break,
            Err(e) => panic!("заполнение сокета: {e:?}"),
        }
    }
    let mut pending = PendingPacket::new(b"pending".to_vec()).unwrap();
    assert!(matches!(
        pending.attempt(&sender, true),
        Err(TransportError::WouldBlock)
    ));
    assert_eq!(pending.state(), SendState::Prepared);
    assert!(matches!(
        pending.attempt(&sender, false),
        Err(TransportError::Invalid)
    ));
    assert_eq!(pending.state(), SendState::Revoked);
    while let Ok(packet) = receiver.receive() {
        assert_eq!(packet, filler);
    }
    assert!(matches!(
        pending.attempt(&sender, true),
        Err(TransportError::Invalid)
    ));
    assert!(matches!(
        receiver.receive(),
        Err(TransportError::WouldBlock)
    ));
}
