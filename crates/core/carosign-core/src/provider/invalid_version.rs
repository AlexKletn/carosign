use core::fmt;

/// That's not a good version.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidVersion;

impl fmt::Debug for InvalidVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidVersion")
    }
}

impl fmt::Display for InvalidVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("версия пакета: ожидается «число.число.число», до 64 байт")
    }
}

impl core::error::Error for InvalidVersion {}
