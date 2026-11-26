use carosign_core::device::DeviceLocator;

/// One device from the system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceEntry {
    /// Temporary address.
    pub locator: DeviceLocator,
    /// Signs for comparison: subsystem, VID, PID and so on.
    pub signals: Vec<(String, String)>,
}
