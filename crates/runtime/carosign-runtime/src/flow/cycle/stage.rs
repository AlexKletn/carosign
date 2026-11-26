use core::fmt;

/// Where the cycle stopped.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Stage {
    /// Nothing's started.
    Idle,
    /// Authorization requested.
    Authorized,
    /// The set of authorized registrations is accepted.
    Scoped,
    /// Operation set up.
    Created,
    /// Command dispatched.
    Dispatched,
    /// Result received.
    Settled,
    /// Refusal.
    Refused,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Idle => "не начато",
            Self::Authorized => "полномочия",
            Self::Scoped => "набор регистраций",
            Self::Created => "операция создана",
            Self::Dispatched => "отправлено",
            Self::Settled => "итог",
            Self::Refused => "отказ",
        })
    }
}
