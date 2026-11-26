use core::fmt;

/// The name is not good: empty, long, not ASCII or reverse domain.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidPackageName;

impl fmt::Debug for InvalidPackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidPackageName")
    }
}

impl fmt::Display for InvalidPackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "имя пакета: нужно 2+ части обратного домена, ASCII в нижнем регистре, до 128 байт",
        )
    }
}

impl core::error::Error for InvalidPackageName {}
