use super::DeviceEntry;
use carosign_core::device::DeviceLocator;

/// A snapshot of inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventoryScan {
    entries: Vec<DeviceEntry>,
    /// Was there an error in the collection (incomplete inventory)?
    incomplete: bool,
}

impl InventoryScan {
    /// Empty inventory is legal: there may be no devices on the car.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Collects devices from `sysfs`.
    ///
    /// Currently, the source collects only class devices. `hidraw`.
    /// The rest of the inventory sources are connected separately.
    ///
    /// ⚠️ Uncompleted roundabout marked `incomplete`It's not a full list, it's a blank list,
    /// and "we couldn't see" are different statements.
    pub fn scan() -> Self {
        let mut scan = Self::default();
        if !scan.collect_hidraw() {
            scan.incomplete = true;
        }
        scan
    }

    fn collect_hidraw(&mut self) -> bool {
        let dir = "/sys/class/hidraw";
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        self.collect_hidraw_entries(entries)
    }

    pub(super) fn collect_hidraw_entries(
        &mut self,
        entries: impl IntoIterator<Item = std::io::Result<std::fs::DirEntry>>,
    ) -> bool {
        let mut complete = true;
        for result in entries {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => {
                    complete = false;
                    continue;
                }
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                complete = false;
                continue;
            };
            let device_dir = entry.path().join("device");
            let mut signals = vec![("subsystem".to_string(), "hidraw".to_string())];
            if let Some(vendor) = read_attr(&device_dir.join("idVendor")) {
                signals.push(("usb.idVendor".to_string(), vendor));
            }
            if let Some(product) = read_attr(&device_dir.join("idProduct")) {
                signals.push(("usb.idProduct".to_string(), product));
            }
            if let Some(uevent) = read_attr(&device_dir.join("uevent")) {
                for line in uevent.lines() {
                    if let Some((key, value)) = line.split_once('=')
                        && (key == "HID_NAME" || key == "HID_ID")
                    {
                        signals.push((format!("hid.{key}"), value.to_string()));
                    }
                }
            } else {
                // Disappearance of the device or error reading its mandatory uevent does not
                // prove the absence of features in the system.
                complete = false;
            }
            let Ok(locator) = DeviceLocator::new(&format!("/dev/{name}")) else {
                complete = false;
                continue;
            };
            self.entries.push(DeviceEntry { locator, signals });
        }
        complete
    }

    /// I can see that.
    pub fn entries(&self) -> &[DeviceEntry] {
        &self.entries
    }

    /// Is the picture full?
    pub fn is_incomplete(&self) -> bool {
        self.incomplete
    }

    /// How many devices.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Are there devices?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Gives devices as pairs of "address - signs".
    ///
    /// ⚠️ Translation to the matching form makes the adapter (`carosign-adapters`), not this
    /// cray: `carosign-linux` Do not need to know runtime (Document 02, DAG).
    pub fn devices(&self) -> &[DeviceEntry] {
        &self.entries
    }
}

fn read_attr(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}
