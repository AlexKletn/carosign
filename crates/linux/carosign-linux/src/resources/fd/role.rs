use super::HandleKind;

/// The role for which the descriptor is issued.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Role {
    Device,
    /// Notifications.
    Event,
    /// Customization.
    Config,
    /// The mediator's channel.
    MediatedService,
}

impl Role {
    /// The kind of carrier for the role.
    pub fn kind(self) -> HandleKind {
        match self {
            Self::Device => HandleKind::CharacterDevice,
            Self::Event => HandleKind::EventFd,
            Self::Config => HandleKind::SealedMemfd,
            Self::MediatedService => HandleKind::SocketPair,
        }
    }
}
