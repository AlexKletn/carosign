use core::fmt;

/// Adapter failure prior to service.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AdapterError {
    /// Empty name.
    EmptyUser,
    /// Too long a name or service name.
    TooLong,
    /// Control signs.
    ControlCharacters,
    /// Incorrect identifier or purpose of verification.
    InvalidOperation,
}

impl fmt::Debug for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyUser => "AdapterError::EmptyUser",
            Self::TooLong => "AdapterError::TooLong",
            Self::ControlCharacters => "AdapterError::ControlCharacters",
            Self::InvalidOperation => "AdapterError::InvalidOperation",
        })
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyUser => "пустое имя пользователя",
            Self::TooLong => "слишком длинное поле",
            Self::ControlCharacters => "управляющие знаки в поле",
            Self::InvalidOperation => "некорректная проверка",
        })
    }
}

impl core::error::Error for AdapterError {}
