use super::*;

#[test]
fn version_range_is_intersected_without_enumerating_peer_values() {
    assert_eq!(
        negotiate(limits::MAJOR, 0, u32::MAX).unwrap().minor,
        limits::MINOR_MAX
    );
    assert!(negotiate(limits::MAJOR, u32::MAX, u32::MAX).is_err());
    assert!(negotiate(limits::MAJOR, 4, 3).is_err());
    assert!(negotiate(limits::MAJOR + 1, 0, u32::MAX).is_err());
}
