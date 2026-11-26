use core::fmt;

/// The label doesn't work.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidLabel;

impl fmt::Debug for InvalidLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidLabel")
    }
}

impl fmt::Display for InvalidLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("метка регистрации длиннее 64 байт")
    }
}

impl core::error::Error for InvalidLabel {}
