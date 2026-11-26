use core::fmt;

/// How the resource is shared among owners.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessMode {
    /// One owner at a time.
    Exclusive,
    /// A few readers; you can't change.
    SharedRead,
    /// Access only through a protocol broker.
    Mediated,
}

impl fmt::Debug for AccessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exclusive => "Exclusive",
            Self::SharedRead => "SharedRead",
            Self::Mediated => "Mediated",
        })
    }
}
