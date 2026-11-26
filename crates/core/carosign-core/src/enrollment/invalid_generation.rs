use core::fmt;

/// Generation is invalid: zero or overflow.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidGeneration;

impl fmt::Debug for InvalidGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidGeneration")
    }
}

impl fmt::Display for InvalidGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("поколение регистрации: ноль недействителен, переполнение запрещено")
    }
}

impl core::error::Error for InvalidGeneration {}
