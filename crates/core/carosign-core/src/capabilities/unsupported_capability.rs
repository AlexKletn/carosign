use core::fmt;

/// The ability is not maintained or the value is unknown.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedCapability {
    /// The value of listing outside baseline.
    Value(i32),
    /// The possibility is not declared by the provider or is not understood by the carrier.
    NotOffered,
}

impl fmt::Debug for UnsupportedCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(v) => write!(f, "UnsupportedCapability::Value({v})"),
            Self::NotOffered => f.write_str("UnsupportedCapability::NotOffered"),
        }
    }
}

impl fmt::Display for UnsupportedCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(v) => write!(f, "способность со значением {v} не поддерживается"),
            Self::NotOffered => f.write_str("способность не заявлена"),
        }
    }
}

impl core::error::Error for UnsupportedCapability {}
