use core::fmt;

/// Refusal of extradition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DelegationError {
    /// The kind of descriptor doesn't fit the role.
    WrongKind,
    /// More rights than necessary.
    TooBroad,
    /// It's not the era.
    StaleEpoch,
}

impl fmt::Display for DelegationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WrongKind => "вид описателя не подходит роли",
            Self::TooBroad => "права шире, чем нужно для роли",
            Self::StaleEpoch => "эпоха не та",
        })
    }
}

impl core::error::Error for DelegationError {}
