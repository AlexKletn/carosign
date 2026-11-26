use super::*;

#[test]
fn config_is_a_sealed_readonly_snapshot() {
    use std::io::{Read, Seek, SeekFrom};
    let directory = std::env::current_dir()
        .unwrap()
        .join(format!(".config-snapshot-test-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("input");
    std::fs::write(&path, b"original").unwrap();
    let resource = ConfiguredResource {
        path: path.clone(),
        resource_id: [3; 16],
        role: 3,
    };
    let fd = open_resource(&resource).unwrap();
    assert!(carosign_linux::sys::is_sealed(&fd).unwrap());
    std::fs::write(&path, b"modified").unwrap();
    let mut snapshot = std::fs::File::from(fd);
    snapshot.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    snapshot.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn received_device_must_be_same_node_and_read_write() {
    let resource = ConfiguredResource {
        path: "/dev/null".into(),
        resource_id: [7; 16],
        role: 1,
    };
    let read_only: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    assert!(validate_owned_resource(&resource, &read_only).is_err());
    let wrong: OwnedFd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/zero")
        .unwrap()
        .into();
    assert!(validate_owned_resource(&resource, &wrong).is_err());
    let correct: OwnedFd = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .unwrap()
        .into();
    assert!(validate_owned_resource(&resource, &correct).is_ok());
}
