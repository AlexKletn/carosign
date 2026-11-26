use std::os::fd::OwnedFd;

/// Couple of connected sockets `SOCK_SEQPACKET`.
///
/// ⚠️ One end goes to the worker, the other one stays with us. `BindAuthenticator`.
#[derive(Debug)]
pub struct SocketPair {
    /// End of platform.
    pub platform: OwnedFd,
    /// The end of the worker.
    pub worker: OwnedFd,
}
