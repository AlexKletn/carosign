use core::fmt;

/// What is proved: only a program or a path through a secure device.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssuranceClass {
    /// The match is calculated by the program outside the protected module.
    SoftwareAsserted,
    /// The evidence was obtained by a protected environment and verified independently.
    HardwareVerified,
    /// The examiner was unable to establish the origin - the worst outcome.
    Unverified,
}

impl fmt::Debug for AssuranceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SoftwareAsserted => "SoftwareAsserted",
            Self::HardwareVerified => "HardwareVerified",
            Self::Unverified => "Unverified",
        })
    }
}
