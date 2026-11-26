use super::*;
use carosign_core::ids::ResourceId;

#[test]
fn reads_empty_and_retained_fence_from_real_database() {
    let dir = std::env::temp_dir().join(format!("carosign-fence-inspect-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("fences.redb");
    drop(InitialFenceStore::create_new(&path).unwrap());
    assert_eq!(count(&path).unwrap(), 0);
    let store = InitialFenceStore::open_guarded(&path).unwrap();
    store
        .fence(ResourceId::from_bytes([1; 16]).unwrap())
        .unwrap();
    drop(store);
    assert_eq!(count(&path).unwrap(), 1);
    assert!(count(&dir.join("missing")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
