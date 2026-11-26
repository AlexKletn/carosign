use core::fmt;

/// The hint is not suitable: empty, long or with control signs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidAccountHint;

impl fmt::Debug for InvalidAccountHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidAccountHint")
    }
}

impl fmt::Display for InvalidAccountHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("подсказка учётной записи: пустая, длиннее 64 байт или с управляющими знаками")
    }
}

impl core::error::Error for InvalidAccountHint {}
