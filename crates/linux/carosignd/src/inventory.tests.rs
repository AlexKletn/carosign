use super::*;
use carosign_core::device::DeviceLocator;
use carosign_linux::discovery::inventory::DeviceEntry;

#[test]
fn complete_empty_scan_remains_empty_and_incomplete_is_not_empty_success() {
    let inventory = from_linux_scan(&InventoryScan::empty()).unwrap();
    assert!(inventory.is_empty());
    assert_eq!(convert(true, &[]), Err(IncompleteInventory));
}

#[test]
fn conversion_keeps_locator_and_all_signals_without_choosing_provider() {
    let entry = DeviceEntry {
        locator: DeviceLocator::new("/dev/hidraw-test").unwrap(),
        signals: vec![
            ("subsystem".into(), "hidraw".into()),
            ("hid.HID_ID".into(), "0003:0001:0002".into()),
        ],
    };
    let inventory = convert(false, std::slice::from_ref(&entry)).unwrap();
    assert_eq!(inventory.entries()[0].locator, entry.locator);
    assert_eq!(inventory.entries()[0].signals, entry.signals);
}
