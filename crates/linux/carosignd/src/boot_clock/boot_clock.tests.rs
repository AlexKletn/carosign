use super::*;
use carosign_core::operation::context::BootDeadline;

#[test]
fn kernel_clock_has_current_boot_and_monotonic_time() {
    let clock = BootClock::initialize().unwrap();
    assert_eq!(clock.boot_id().as_bytes(), &sys::boot_id().unwrap());
    let first = clock.now();
    let second = clock.now();
    assert_eq!(first.boot(), clock.boot_id());
    assert!(second.instant_ns() >= first.instant_ns());
    assert_ne!(second.instant_ns(), u64::MAX);
}

#[test]
fn error_and_backwards_time_permanently_expire_deadlines() {
    let boot = BootId::from_bytes([1; 16]).unwrap();
    for failure in [Err(SysError { errno: libc::EIO }), Ok(9)] {
        let mut state = ClockState {
            last: 10,
            failed: false,
        };
        let expired = state.observe(failure);
        assert!(BootDeadline::new(boot, 100).is_expired_at(BootInstant::new(boot, expired)));
        assert_eq!(state.observe(Ok(11)), u64::MAX);
    }
}
