use super::*;

#[test]
fn epoch_zero_is_invalid() {
    assert!(DeviceEpoch::new(0).is_err());
    assert_eq!(DeviceEpoch::new(1), Ok(DeviceEpoch::FIRST));
}

#[test]
fn epoch_overflow_is_a_hard_error() {
    // A mistake should sound like a mistake, not be wrapped in zero.
    let last = DeviceEpoch::new(MAX_COUNTER).expect("валид");
    assert!(last.next().is_err());
    assert!(DeviceEpoch::FIRST.next().is_ok());
}

#[test]
fn locator_is_bounded() {
    assert!(DeviceLocator::new("/dev/hidraw0").is_ok());
    assert!(DeviceLocator::new("").is_err());
    assert!(DeviceLocator::new(&"x".repeat(257)).is_err());
    assert!(DeviceLocator::new("bad\u{0007}path").is_err());
}
