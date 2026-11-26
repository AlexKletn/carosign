use super::UnsupportedCapability;
use core::fmt;

/// How can a changing state be restored later?
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecoveryMode {
    /// There is no way – the area remains blocked.
    NotSupported,
    /// Secure status request.
    SafeStateQuery,
    /// Recovery on the provider side.
    ProviderManaged,
}

impl fmt::Debug for RecoveryMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotSupported => "NotSupported",
            Self::SafeStateQuery => "SafeStateQuery",
            Self::ProviderManaged => "ProviderManaged",
        })
    }
}

impl RecoveryMode {
    pub fn from_wire(value: i32) -> Result<Self, UnsupportedCapability> {
        match value {
            1 => Ok(Self::NotSupported),
            2 => Ok(Self::SafeStateQuery),
            3 => Ok(Self::ProviderManaged),
            other => Err(UnsupportedCapability::Value(other)),
        }
    }

    /// Is there a way to reconcile at all?
    pub fn can_reconcile(self) -> bool {
        !matches!(self, Self::NotSupported)
    }
}
