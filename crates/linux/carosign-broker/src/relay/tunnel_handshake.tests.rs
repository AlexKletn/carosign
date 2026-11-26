use super::super::test_fixture::hello_frame;
use super::*;
use prost::Message;

#[test]
fn handshake_reads_real_packet_and_closes_on_replay() {
    let pair = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let peer = PacketSocket::new(pair.platform).unwrap();
    let mut handshake =
        TunnelHandshake::new(PacketSocket::new(pair.worker).unwrap(), [1; 16], 1).unwrap();
    assert!(matches!(handshake.poll(), Err(TransportError::WouldBlock)));
    peer.send(&hello_frame().encode_to_vec()).unwrap();
    assert_eq!(handshake.poll().unwrap(), ([2; 16], [3; 16]));
    peer.send(&hello_frame().encode_to_vec()).unwrap();
    assert!(matches!(handshake.poll(), Err(TransportError::Invalid)));
    assert!(matches!(peer.receive(), Err(TransportError::Closed)));
}

#[test]
fn hello_transfers_the_same_socket_to_the_pump() {
    let tunnel = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let provider = carosign_linux::sys::socketpair_seqpacket().unwrap();
    let daemon = PacketSocket::new(tunnel.platform).unwrap();
    let worker = PacketSocket::new(provider.worker).unwrap();
    let mut handshake =
        TunnelHandshake::new(PacketSocket::new(tunnel.worker).unwrap(), [1; 16], 1).unwrap();
    daemon.send(&hello_frame().encode_to_vec()).unwrap();
    handshake.poll().unwrap();
    let mut pump = handshake
        .into_pump(PacketSocket::new(provider.platform).unwrap())
        .unwrap();
    // These bytes represent already verified external caller frames.
    pump.command(b"command".to_vec()).unwrap();
    assert!(pump.send_command(true).unwrap());
    assert_eq!(worker.receive().unwrap(), b"command");
    pump.event(b"event".to_vec()).unwrap();
    assert!(pump.send_event().unwrap());
    assert_eq!(daemon.receive().unwrap(), b"event");
    pump.revoke();
    assert!(matches!(daemon.receive(), Err(TransportError::Closed)));
    assert!(matches!(worker.receive(), Err(TransportError::Closed)));
}
