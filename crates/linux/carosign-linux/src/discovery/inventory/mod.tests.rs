use super::*;

#[test]
fn empty_inventory_is_legal() {
    let scan = InventoryScan::empty();
    assert!(scan.is_empty());
    assert!(!scan.is_incomplete());
}

#[test]
fn enumeration_error_does_not_become_a_complete_empty_scan() {
    let mut scan = InventoryScan::empty();
    let result = scan.collect_hidraw_entries(std::iter::once(Err(std::io::Error::other(
        "ошибка чтения каталога",
    ))));
    assert!(!result);
    assert!(scan.entries().is_empty());
}

#[test]
fn unreadable_device_uevent_does_not_yield_a_complete_inventory() {
    let directory = std::env::temp_dir().join(format!(
        "carosign-inventory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    std::fs::create_dir(directory.join("hidraw0")).unwrap();
    let mut scan = InventoryScan::empty();
    let complete = scan.collect_hidraw_entries(std::fs::read_dir(&directory).unwrap());
    assert!(!complete);
    assert_eq!(scan.entries().len(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
