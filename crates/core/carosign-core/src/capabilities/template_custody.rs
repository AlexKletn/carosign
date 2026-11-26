use super::UnsupportedCapability;
use core::fmt;

/// Who keeps the pattern.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateCustody {
    /// Platform (encrypted template in metadata storage).
    PlatformManaged,
    /// Provider or secure module.
    ProviderManaged,
}

impl fmt::Debug for TemplateCustody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PlatformManaged => "PlatformManaged",
            Self::ProviderManaged => "ProviderManaged",
        })
    }
}

impl TemplateCustody {
    /// Value of wire `carosign.common.v1.TemplateCustody`.
    pub fn from_wire(value: i32) -> Result<Self, UnsupportedCapability> {
        match value {
            1 => Ok(Self::PlatformManaged),
            2 => Ok(Self::ProviderManaged),
            other => Err(UnsupportedCapability::Value(other)),
        }
    }
}
