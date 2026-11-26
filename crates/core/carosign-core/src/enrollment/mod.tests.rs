use crate::ids::AuthenticatorId;
use crate::ids::EnrollmentId;

use super::*;

#[test]
fn generation_overflow_is_an_error_and_no_mutation_follows() {
    // C-03: Generation overflow is a mistake, change does not begin.
    let last = EnrollmentGeneration::new(u64::MAX).expect("валид");
    let outcome = last.next();
    assert!(outcome.is_err());
    // The generation over which the next generation could not be built remained intact.
    assert_eq!(last.value(), u64::MAX);
}

#[test]
fn generation_starts_at_one() {
    assert!(EnrollmentGeneration::new(0).is_err());
    assert_eq!(
        EnrollmentGeneration::new(1),
        Ok(EnrollmentGeneration::FIRST)
    );
    assert_eq!(
        EnrollmentGeneration::FIRST
            .next()
            .expect("не переполнено")
            .value(),
        2
    );
}

#[test]
fn label_is_bounded_in_utf8_bytes() {
    assert!(EnrollmentLabel::new("").is_ok());
    assert!(EnrollmentLabel::new("указательный").is_ok());
    assert!(EnrollmentLabel::new(&"я".repeat(33)).is_err()); // 33 × 2 = 66 bytes
    assert!(EnrollmentLabel::new(&"a".repeat(64)).is_ok());
    assert_eq!(
        EnrollmentLabel::new("a\u{001b}[31m").unwrap().as_str(),
        "a\u{001b}[31m"
    );
}

#[test]
fn opaque_ref_is_bounded() {
    assert!(OpaqueProviderRef::new(Vec::new()).is_ok());
    assert!(OpaqueProviderRef::new(vec![7u8; OPAQUE_REFERENCE_MAX]).is_ok());
    assert!(OpaqueProviderRef::new(vec![7u8; OPAQUE_REFERENCE_MAX + 1]).is_err());
}

#[test]
fn enrollment_ref_pins_generation() {
    let id = EnrollmentId::from_bytes([1u8; 16]).expect("валид");
    let auth = AuthenticatorId::from_bytes([2u8; 16]).expect("валид");
    let first = EnrollmentRef {
        id,
        authenticator: auth,
        generation: EnrollmentGeneration::FIRST,
    };
    let second = EnrollmentRef {
        id,
        authenticator: auth,
        generation: EnrollmentGeneration::FIRST.next().expect("не переполнено"),
    };
    assert_ne!(first, second);
}
