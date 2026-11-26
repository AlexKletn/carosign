use super::*;

#[test]
fn staging_location_revalidates_components_and_inode() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root_path = std::env::temp_dir().join(format!(
        "carosign-staging-location-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir(&root_path).unwrap();
    std::fs::set_permissions(&root_path, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(root_path.join("versions")).unwrap();
    std::fs::create_dir(root_path.join("versions/one")).unwrap();
    let root = std::fs::File::open(&root_path).unwrap();
    let staging = std::fs::File::open(root_path.join("versions/one")).unwrap();
    let owner = unsafe { libc::geteuid() };
    let location =
        StagingLocation::bind(root.as_fd(), staging.as_fd(), "versions/one", owner).unwrap();
    for path in [
        "",
        "/versions/one",
        "versions//one",
        "versions/../one",
        "versions/./one",
    ] {
        assert!(StagingLocation::bind(root.as_fd(), staging.as_fd(), path, owner).is_err());
    }
    std::fs::set_permissions(
        root_path.join("versions"),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(location.revalidate().is_err());
    std::fs::set_permissions(
        root_path.join("versions"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    location.revalidate().unwrap();
    std::fs::rename(
        root_path.join("versions/one"),
        root_path.join("versions/original"),
    )
    .unwrap();
    std::fs::create_dir(root_path.join("versions/one")).unwrap();
    assert!(location.revalidate().is_err());
    std::fs::remove_dir(root_path.join("versions/one")).unwrap();
    symlink("original", root_path.join("versions/one")).unwrap();
    assert!(location.revalidate().is_err());
    drop(location);
    drop(staging);
    drop(root);
    std::fs::remove_dir_all(root_path).unwrap();
}
