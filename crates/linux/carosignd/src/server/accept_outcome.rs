use super::{PeerUser, fmt};

/// How did the reception end?
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AcceptOutcome {
    /// Connection accepted, session set.
    Accepted {
        /// Who's here?
        peer: PeerUser,
        /// Connection identifier (not authority).
        connection: [u8; 16],
    },
    /// The core failed.
    Kernel(i32),
    /// Credents are not removed - the connection is closed.
    NoCredentials,
}

impl fmt::Display for AcceptOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted { peer, .. } => write!(f, "принято от uid {}", peer.uid),
            Self::Kernel(errno) => write!(f, "ядро отказало: {errno}"),
            Self::NoCredentials => f.write_str("креды недоступны"),
        }
    }
}
