use super::InvalidLocator;
use core::fmt;

/// The maximum length of the device's temporary address, in bytes.
pub const DEVICE_LOCATOR_MAX: usize = 256;

/// Temporary address of the device from the inventory.
///
/// ⚠️ **No identity and no base key: the address changes when you reconnect, and the
/// registrations remain.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceLocator(String);

impl DeviceLocator {
    /// Accepts the address as it is - only for comparison during the search.
    pub fn new(value: &str) -> Result<Self, InvalidLocator> {
        if value.is_empty() || value.len() > DEVICE_LOCATOR_MAX {
            return Err(InvalidLocator);
        }
        if value.chars().any(char::is_control) {
            return Err(InvalidLocator);
        }
        Ok(Self(value.to_owned()))
    }

    /// The address is what the inventory called it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DeviceLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceLocator(..)")
    }
}
