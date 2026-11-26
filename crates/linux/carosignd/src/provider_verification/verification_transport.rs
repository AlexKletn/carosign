use super::{Duration, ExchangeError, ProviderFrame, ProviderTransport, tunnel_error};

/// Single ownership of a two-way verification channel.
/// The origin of the mediator and powers are verified by the caller separately.
pub enum VerificationTransport {
    /// Direct channel used by the existing test staff.
    Direct(ProviderTransport),
    /// The channel is through an broker; an incomprehensible outcome closes the tunnel.
    Tunnel(Box<carosign_linux_ipc::provider::tunnel::TunnelClient>),
}

impl VerificationTransport {
    pub(super) fn send(
        &mut self,
        frame: ProviderFrame,
        timeout: Duration,
    ) -> Result<(), ExchangeError> {
        match self {
            Self::Direct(channel) => channel.send(frame, timeout),
            Self::Tunnel(channel) => channel.command(frame, timeout).map_err(tunnel_error),
        }
    }

    pub(super) fn receive(&mut self, timeout: Duration) -> Result<ProviderFrame, ExchangeError> {
        match self {
            Self::Direct(channel) => channel.receive(timeout),
            Self::Tunnel(channel) => channel.receive_event(timeout).map_err(tunnel_error),
        }
    }

    pub(super) fn abort(&mut self) {
        match self {
            Self::Direct(channel) => channel.abort(),
            Self::Tunnel(channel) => channel.abort(),
        }
    }
}
