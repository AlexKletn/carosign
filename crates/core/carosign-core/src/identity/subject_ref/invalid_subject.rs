use core::fmt;

/// Canonical recording of a subject of incorrect length or with zeros.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidSubject;

impl fmt::Debug for InvalidSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InvalidSubject")
    }
}

impl fmt::Display for InvalidSubject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("субъект: неверная длина записи или нулевой идентификатор")
    }
}

impl core::error::Error for InvalidSubject {}
