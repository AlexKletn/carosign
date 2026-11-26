use carosign_provider_sdk::wire::TransportError;

/// Failure to exchange: After an I/O error, the channel cannot be reused.
#[derive(Debug)]
pub enum ExchangeError {
    /// The channel is not connected to Unix seqpacket.
    InvalidSocket,
    /// Error of forming or verifying the protocol.
    Protocol(TransportError),
    /// It's over.
    Timeout,
    /// The channel is closed or the core operation failed.
    Io(std::io::Error),
    /// A cropped packet or unexpected ancillary data came in.
    Truncated,
    /// Descriptors require a separate confirmed Bind path.
    DescriptorsUnsupported,
    /// The channel no longer allows exchange.
    Closed,
}
