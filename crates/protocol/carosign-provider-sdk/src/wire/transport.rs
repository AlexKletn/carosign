use super::transport_error::TransportError;

/// Channel to the platform.
///
/// ⚠️ Implemented by each media separately: in Linux `SOCK_SEQPACKET` The SDK process, the
/// checks, the dummy. The SDK doesn't know whose channel it is.
pub trait Transport {
    /// Reads one message in its entirety.
    ///
    /// ⚠️ The cliff returns as `Ok(None)`Not as a blank frame: an empty frame is also a
    /// message (and usually a mistake).
    fn receive(&mut self) -> Result<Option<Vec<u8>>, TransportError>;

    /// Sends one message in its entirety.
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
}
