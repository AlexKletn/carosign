use core::fmt;

/// What a descriptor.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandleKind {
    /// It's a symbol device.
    CharacterDevice,
    /// The couple's socket (the mediator's channel).
    SocketPair,
    /// File in memory with a seal.
    SealedMemfd,
    /// Event counter.
    EventFd,
}

impl fmt::Debug for HandleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CharacterDevice => "CharacterDevice",
            Self::SocketPair => "SocketPair",
            Self::SealedMemfd => "SealedMemfd",
            Self::EventFd => "EventFd",
        })
    }
}
