use core::fmt;

/// Kind of resource.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    /// It's a symbol device.
    CharacterDevice,
    /// Couple's socket (at the middleman's).
    SocketPair,
    /// File's in memory.
    SealedMemfd,
    /// Event counter.
    EventFd,
}

impl fmt::Debug for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CharacterDevice => "CharacterDevice",
            Self::SocketPair => "SocketPair",
            Self::SealedMemfd => "SealedMemfd",
            Self::EventFd => "EventFd",
        })
    }
}
