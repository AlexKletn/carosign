use core::fmt;

/// Points at which authority is re-examined (document 08).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthorizationBoundary {
    /// B1 – before creating an operation and seizing resources.
    BeforeCreation,
    /// B2 – Before you start checking or sending a changing command.
    BeforeDispatch,
    /// B3 - after matching, before delivering the result to the consumer.
    BeforeDelivery,
    /// B4 - before fixing the profile binding (for the platform adapter).
    BeforeBinding,
}

impl fmt::Debug for AuthorizationBoundary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BeforeCreation => "B1:BeforeCreation",
            Self::BeforeDispatch => "B2:BeforeDispatch",
            Self::BeforeDelivery => "B3:BeforeDelivery",
            Self::BeforeBinding => "B4:BeforeBinding",
        })
    }
}
