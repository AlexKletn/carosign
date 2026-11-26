use super::*;

#[test]
fn empty_and_truncated_are_refused() {
    assert_eq!(check(&[], false), Err(FrameError::Empty));
    assert_eq!(check(b"x", true), Err(FrameError::Truncated));
}

#[test]
fn oversized_is_refused_before_parsing() {
    let big = vec![0u8; PACKET_MAX + 1];
    assert!(matches!(
        check(&big, false),
        Err(FrameError::TooLarge { .. })
    ));
}

#[test]
fn descriptor_count_must_match_slots() {
    assert!(check_descriptors(2, 2).is_ok());
    assert!(check_descriptors(2, 1).is_err());
    assert!(check_descriptors(1, 2).is_err());
    assert!(check_descriptors(9, 9).is_err());
}
