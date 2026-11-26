use core::fmt;

/// Unsubscribe.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ObserverError {
    /// More than the limit.
    TooMany,
    /// There's already a number.
    Duplicate,
}

impl fmt::Debug for ObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooMany => "ObserverError::TooMany",
            Self::Duplicate => "ObserverError::Duplicate",
        })
    }
}

impl fmt::Display for ObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::TooMany => "наблюдателей сверх предела",
            Self::Duplicate => "наблюдатель с таким номером уже есть",
        })
    }
}

impl core::error::Error for ObserverError {}
