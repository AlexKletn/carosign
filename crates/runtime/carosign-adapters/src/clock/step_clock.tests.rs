use super::*;

fn boot() -> BootId {
    BootId::from_bytes([1u8; 16]).expect("валид")
}

#[test]
fn step_clock_only_moves_when_told() {
    let clock = StepClock::new(boot());
    let first = clock.now();
    assert_eq!(clock.now(), first);
    clock.advance(5);
    assert!(clock.now().instant_ns() > first.instant_ns());
}

#[test]
fn step_clock_never_wraps_backwards() {
    let clock = StepClock::new(boot());
    clock.advance(u64::MAX - 1);
    clock.advance(10);
    assert_eq!(clock.now().instant_ns(), u64::MAX);
    clock.advance(1);
    assert_eq!(clock.now().instant_ns(), u64::MAX);
}
