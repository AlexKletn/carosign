use carosign_core::device::DeviceLocator;

/// One device from the inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryEntry {
    /// Temporary address.
    pub locator: DeviceLocator,
    /// Signs for comparison (key → value).
    pub signals: Vec<(String, String)>,
}
