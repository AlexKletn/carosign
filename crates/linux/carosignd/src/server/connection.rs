use super::{ClientSession, PeerUser};

/// Accepted connection together with the session.
#[derive(Debug)]
pub struct Connection {
    pub session: ClientSession,
    /// Who's here?
    pub peer: PeerUser,
    /// Connection descriptor.
    pub fd: std::os::fd::OwnedFd,
}
