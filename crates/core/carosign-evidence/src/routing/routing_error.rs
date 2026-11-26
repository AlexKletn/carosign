use core::fmt;

/// Refusal of delivery.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RoutingError {
    /// It's already been presented.
    AlreadyConsumed,
    WrongOperation,
}

impl fmt::Debug for RoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AlreadyConsumed => "RoutingError::AlreadyConsumed",
            Self::WrongOperation => "RoutingError::WrongOperation",
        })
    }
}

impl fmt::Display for RoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AlreadyConsumed => "материал уже предъявлялся",
            Self::WrongOperation => "материал принадлежит другой операции",
        })
    }
}

impl core::error::Error for RoutingError {}
