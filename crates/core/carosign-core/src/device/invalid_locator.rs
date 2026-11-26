use core::fmt;

/// The address is not suitable: empty, long or with control signs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidLocator;

impl fmt::Debug for InvalidLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidLocator")
    }
}

impl fmt::Display for InvalidLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("адрес устройства: пустой, длиннее 256 байт или с управляющими знаками")
    }
}

impl core::error::Error for InvalidLocator {}
