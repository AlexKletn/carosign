use super::SendGuard;
use super::invalid_transition::InvalidTransition;
use super::send_state::SendState;

/// One withheld shipment on an worker's non-blocking channel.
///
/// This facility does not verify origin or policy: the caller is required to serialize
/// retesting authority and recall with a call. `attempt`.
/// In EAGAIN, the packet remains the same; in another error, repetition is prohibited.
#[allow(dead_code)]
pub(crate) struct PendingPacket {
    bytes: Vec<u8>,
    bind_fds: Option<Vec<std::os::fd::OwnedFd>>,
    guard: SendGuard,
}

#[allow(dead_code)]
impl PendingPacket {
    pub(crate) fn new(bytes: Vec<u8>) -> Result<Self, crate::transport::TransportError> {
        if bytes.is_empty() || bytes.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(crate::transport::TransportError::Invalid);
        }
        Ok(Self {
            bytes,
            bind_fds: None,
            guard: SendGuard::new(),
        })
    }

    pub(crate) fn bind(
        bytes: Vec<u8>,
        fds: Vec<std::os::fd::OwnedFd>,
    ) -> Result<Self, crate::transport::TransportError> {
        let mut packet = Self::new(bytes)?;
        packet.bind_fds = Some(fds);
        Ok(packet)
    }

    pub(crate) fn state(&self) -> SendState {
        self.guard.state()
    }

    pub(crate) fn revoke(&mut self) -> Result<(), InvalidTransition> {
        self.guard.revoke()
    }

    /// Sending it exactly once. `authorized` Received by re-checking independent sources
    /// under the same serialization as the review.
    pub(crate) fn attempt(
        &mut self,
        socket: &crate::transport::PacketSocket,
        authorized: bool,
    ) -> Result<(), crate::transport::TransportError> {
        use crate::transport::TransportError;
        if !authorized {
            self.guard.revoke().map_err(|_| TransportError::Invalid)?;
            return Err(TransportError::Invalid);
        }
        self.guard.begin().map_err(|_| TransportError::Invalid)?;
        let sent = match &self.bind_fds {
            Some(fds) => {
                use prost::Message;
                use std::os::fd::AsFd;
                let frame = carosign_protocol::carosign::provider::v1::ProviderFrame::decode(
                    self.bytes.as_slice(),
                )
                .map_err(|_| TransportError::Invalid)?;
                socket.send_bind(&frame, &fds.iter().map(|fd| fd.as_fd()).collect::<Vec<_>>())
            }
            None => socket.send(&self.bytes),
        };
        match sent {
            Ok(()) => {
                self.guard.sent().map_err(|_| TransportError::Invalid)?;
                Ok(())
            }
            Err(TransportError::WouldBlock) => {
                self.guard
                    .would_block()
                    .map_err(|_| TransportError::Invalid)?;
                Err(TransportError::WouldBlock)
            }
            Err(error) => {
                let _ = self.guard.lost();
                Err(error)
            }
        }
    }
}

#[cfg(test)]
#[path = "pending_packet.tests.rs"]
mod tests;
