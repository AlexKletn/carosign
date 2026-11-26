use core::fmt;

/// The choice of the subject does not even pass syntactic check.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidSelector;

impl fmt::Debug for InvalidSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidSelector")
    }
}

impl fmt::Display for InvalidSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("выбор субъекта: пустой или длиннее 256 байт")
    }
}

impl core::error::Error for InvalidSelector {}
