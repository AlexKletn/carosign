use super::InvalidPackageName;
use core::fmt;

/// The maximum length of the packet name in the reverse domain, in bytes.
pub const PACKAGE_NAME_MAX: usize = 128;

/// Canonical packet name: reverse domain, lower case ASCII.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PackageName(String);

impl PackageName {
    /// Normalizes the value to lowercase before validation.
    ///
    /// Rules: ASCII only `a-z`, `0-9`, `-` and points; not less than two parts; not one part
    /// is empty; not one part begins with a hyphen.
    /// Such a string is a name, not a hardware identity.
    pub fn new(value: &str) -> Result<Self, InvalidPackageName> {
        let lowered = value.to_ascii_lowercase();
        if lowered.len() > PACKAGE_NAME_MAX {
            return Err(InvalidPackageName);
        }
        let parts: Vec<&str> = lowered.split('.').collect();
        if parts.len() < 2 {
            return Err(InvalidPackageName);
        }
        for part in parts {
            if part.is_empty() || part.starts_with('-') || part.ends_with('-') {
                return Err(InvalidPackageName);
            }
            if !part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(InvalidPackageName);
            }
        }
        Ok(Self(lowered))
    }

    /// Canonical name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackageName({})", self.0)
    }
}
