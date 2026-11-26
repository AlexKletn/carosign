use super::InvalidPurpose;
use core::fmt;

/// Why are they checking?
///
/// ⚠️ The value is chosen by the trusted source of authority, not the client: an unverified
/// client prompt becomes the target only after the policy is decided (Document 03, “Purpose
/// is specified by trusted system authority”).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerificationPurpose {
    /// Entrance to the session.
    SessionLogin,
    /// Session unlocking.
    SessionUnlock,
    /// Confirmation of an action requiring rights.
    PrivilegedAction,
    /// Factor self-testing.
    FactorSelfCheck,
    /// Access to a secure key.
    KeyOperation,
}

impl fmt::Debug for VerificationPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SessionLogin => "SessionLogin",
            Self::SessionUnlock => "SessionUnlock",
            Self::PrivilegedAction => "PrivilegedAction",
            Self::FactorSelfCheck => "FactorSelfCheck",
            Self::KeyOperation => "KeyOperation",
        })
    }
}

impl VerificationPurpose {
    /// The value of the wire transfer.
    pub fn from_wire(value: i32) -> Result<Self, InvalidPurpose> {
        match value {
            1 => Ok(Self::SessionLogin),
            2 => Ok(Self::SessionUnlock),
            3 => Ok(Self::PrivilegedAction),
            4 => Ok(Self::FactorSelfCheck),
            5 => Ok(Self::KeyOperation),
            other => Err(InvalidPurpose(other)),
        }
    }

    /// Does it require a hardware-tested warranty?
    ///
    /// The programmatic match does not cover this (Document 11).
    pub fn requires_hardware_assurance(self) -> bool {
        matches!(self, Self::KeyOperation)
    }
}
