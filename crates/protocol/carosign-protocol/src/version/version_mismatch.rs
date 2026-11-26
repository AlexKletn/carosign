/// The inconsistent version.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VersionMismatch {
    /// We have the oldest.
    pub our_major: u32,
    /// They have the oldest.
    pub their_major: u32,
}

impl core::fmt::Display for VersionMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "старшая версия {} не совпадает с {}",
            self.their_major, self.our_major
        )
    }
}

impl core::error::Error for VersionMismatch {}
