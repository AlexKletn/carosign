use core::fmt;

/// The value of modality outside the baseline (e.g., from a future minor wire).
///
/// ⚠️ The unknown value is a **error*, not a default behavior.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct UnknownModality(pub u32);

impl fmt::Debug for UnknownModality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnknownModality({})", self.0)
    }
}

impl fmt::Display for UnknownModality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "модальность {} не поддерживается в v1", self.0)
    }
}

impl core::error::Error for UnknownModality {}
