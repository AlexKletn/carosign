use core::fmt;

/// The role of the descriptor (coinciding with the `carosign.provider.v1.FdRole`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum FdRole {
    Device,
    /// Notifications (eventfd).
    Event,
    /// Unchangeable setting (sealed memfd).
    Config,
    /// Mediation channel (art.`carosign.broker.v1`).
    MediatedService,
}

impl fmt::Debug for FdRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Device => "Device",
            Self::Event => "Event",
            Self::Config => "Config",
            Self::MediatedService => "MediatedService",
        })
    }
}

impl FdRole {
    /// The value of the wire.
    pub fn wire(self) -> i32 {
        match self {
            Self::Device => 1,
            Self::Event => 2,
            Self::Config => 3,
            Self::MediatedService => 4,
        }
    }

    pub fn from_wire(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Device),
            2 => Some(Self::Event),
            3 => Some(Self::Config),
            4 => Some(Self::MediatedService),
            _ => None,
        }
    }
}
