/// Exchange error: If the outcome is unknown, the channel closes completely.
#[derive(Debug)]
pub enum TunnelError {
    /// The socket does not comply with the contract or the system operation ended in a
    /// mistake.
    Io(std::io::Error),
    /// A damaged frame, unsupported result, or contextual disturbance.
    Invalid,
    /// The exchange date has expired; the result cannot be repeated.
    Timeout,
    /// Reuse of the closed channel.
    Closed,
}
