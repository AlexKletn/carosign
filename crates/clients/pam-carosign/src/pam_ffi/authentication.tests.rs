use super::*;

#[test]
fn sudo_retries_empty_sample_only_with_interactive_uncancelled_terminal() {
    assert!(retry_no_sample("sudo", true, false));
    assert!(retry_no_sample("sudo-i", true, false));
    assert!(!retry_no_sample("sudo", false, false));
    assert!(!retry_no_sample("sudo", true, true));
    assert!(!retry_no_sample("gdm-fingerprint", true, false));
}

#[test]
fn sudo_limits_mismatches_to_three_actual_attempts() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let attempts = AtomicUsize::new(1);
    assert!(retry_mismatch(&attempts));
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert!(retry_mismatch(&attempts));
    assert_eq!(attempts.load(Ordering::Relaxed), 3);
    assert!(!retry_mismatch(&attempts));
    assert_eq!(attempts.load(Ordering::Relaxed), 3);
}
