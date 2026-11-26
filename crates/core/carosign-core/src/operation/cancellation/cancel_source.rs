use core::fmt;

/// Who asked me to cancel.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CancelSource {
    /// Owner of the transaction (client).
    Owner,
    /// System policy (revocation, deadline, completion of service).
    System,
    /// The owner's connection record is missing.
    OwnerDisconnected,
}

impl fmt::Debug for CancelSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Owner => "Owner",
            Self::System => "System",
            Self::OwnerDisconnected => "OwnerDisconnected",
        })
    }
}
