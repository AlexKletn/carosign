use super::InvalidLabel;
use core::fmt;

/// The maximum length of the tag that the client sees is in UTF-8 bytes.
pub const ENROLLMENT_LABEL_MAX: usize = 64;

/// Registration name for the client.
///
/// A label is for display. Replacement uses compare-and-swap on the enrollment generation.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct EnrollmentLabel(String);

impl EnrollmentLabel {
    /// Empty label is allowed – it is optional; the limit is 64 bytes of UTF-8.
    pub fn new(value: &str) -> Result<Self, InvalidLabel> {
        if value.len() > ENROLLMENT_LABEL_MAX {
            return Err(InvalidLabel);
        }
        Ok(Self(value.to_owned()))
    }

    /// Like it was passed on.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length in bytes of UTF-8.
    pub fn len_bytes(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for EnrollmentLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnrollmentLabel({} байт)", self.0.len())
    }
}
