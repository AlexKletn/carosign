use core::fmt;

/// Policy rejection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PolicyError {
    /// Repeat.
    Duplicate,
    /// There's no such handler or it's not for that.
    Unsupported,
    /// Not allowed.
    Unauthorized,
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Duplicate => "повтор в политике",
            Self::Unsupported => "обработчик не объявлен",
            Self::Unauthorized => "не разрешено политикой",
        })
    }
}

impl core::error::Error for PolicyError {}
