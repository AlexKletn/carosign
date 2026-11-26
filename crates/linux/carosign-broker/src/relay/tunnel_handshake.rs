use super::HelloGate;
use super::Refusal;
use crate::transport::PacketSocket;
use crate::transport::TransportError;

/// Links the initial analysis of Hello with the owner of the real tunnel-socket.
/// Hello Compliance is not a proof of origin or a right.
#[allow(dead_code)]
pub(super) struct TunnelHandshake {
    socket: PacketSocket,
    gate: HelloGate,
    established: bool,
}

#[allow(dead_code)]
impl TunnelHandshake {
    fn new(socket: PacketSocket, worker: [u8; 16], epoch: u64) -> Result<Self, Refusal> {
        Ok(Self {
            socket,
            gate: HelloGate::new(worker, epoch)?,
            established: false,
        })
    }

    fn poll(&mut self) -> Result<([u8; 16], [u8; 16]), TransportError> {
        let bytes = self.socket.receive()?;
        match self.gate.accept(&bytes, 0) {
            Ok(hello) => {
                self.established = true;
                Ok(hello)
            }
            Err(_) => {
                self.established = false;
                self.socket.close();
                Err(TransportError::Invalid)
            }
        }
    }

    /// After Hello retains ownership of the same tunnel when handed over to the pump.
    /// This does not allow sending or replace checking wire attachments.
    fn into_pump(self, provider: PacketSocket) -> Result<crate::relay_pump::RelayPump, Refusal> {
        if !self.established {
            return Err(Refusal::Invalid);
        }
        Ok(crate::relay_pump::RelayPump::new(provider, self.socket))
    }
}

#[cfg(test)]
#[path = "tunnel_handshake.tests.rs"]
mod tests;
