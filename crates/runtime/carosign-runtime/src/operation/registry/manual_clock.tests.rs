use super::*;
use carosign_core::ids::BootId;

fn boot() -> BootId {
    BootId::from_bytes([5u8; 16]).expect("валид")
}

#[test]
fn manual_clock_only_moves_when_told() {
    let clock = ManualClock::new(boot());
    let first = clock.now();
    assert_eq!(clock.now(), first);
    clock.advance(10);
    assert!(clock.now().instant_ns() > first.instant_ns());
}
