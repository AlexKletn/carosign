use super::InvalidVersion;
use core::fmt;

/// Package version SemVer-lite: three numbers through a dot.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PackageVersion(String);

impl PackageVersion {
    /// Checking the format.
    pub fn new(value: &str) -> Result<Self, InvalidVersion> {
        if value.is_empty() || value.len() > 64 {
            return Err(InvalidVersion);
        }
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() != 3 {
            return Err(InvalidVersion);
        }
        for part in parts {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(InvalidVersion);
            }
        }
        Ok(Self(value.to_owned()))
    }

    /// Like the package announced it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackageVersion({})", self.0)
    }
}
