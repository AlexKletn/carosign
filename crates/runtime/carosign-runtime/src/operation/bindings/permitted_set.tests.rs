use super::super::*;
use carosign_core::enrollment::EnrollmentGeneration;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::ids::AuthenticatorId;
use carosign_core::ids::EnrollmentId;

fn enrollment(byte: u8, generation: u64) -> EnrollmentRef {
    EnrollmentRef {
        id: EnrollmentId::from_bytes([byte; 16]).expect("валид"),
        authenticator: AuthenticatorId::from_bytes([9u8; 16]).expect("валид"),
        generation: EnrollmentGeneration::new(generation).expect("не ноль"),
    }
}

fn binding(byte: u8) -> EnrollmentBinding {
    EnrollmentBinding {
        enrollment: enrollment(byte, 1),
        kind: BindingKind::ProviderOpaque,
        provider_ref: vec![byte; 4],
        format_id: Some("goodix-asp".into()),
        format_version: 1,
    }
}

#[test]
fn empty_set_is_refused() {
    // ⚠️ D-03: "Check someone" doesn't happen.
    assert_eq!(PermittedSet::new(Vec::new()), Err(SetError::Empty));
}

#[test]
fn set_is_bounded_and_unique() {
    let too_many: Vec<_> = (1..=(BINDINGS_MAX as u8 + 1)).map(binding).collect();
    assert!(matches!(
        PermittedSet::new(too_many),
        Err(SetError::TooMany { .. })
    ));
    assert_eq!(
        PermittedSet::new(vec![binding(1), binding(1)]),
        Err(SetError::Duplicate)
    );
    let mut stale = binding(1);
    stale.enrollment.generation = EnrollmentGeneration::new(2).unwrap();
    assert_eq!(
        PermittedSet::new(vec![binding(1), stale]),
        Err(SetError::Duplicate)
    );
    assert!(PermittedSet::new(vec![binding(1), binding(2)]).is_ok());
}

#[test]
fn generation_is_part_of_membership() {
    // ⚠️ Same number, another generation - not "in the set."
    let set = PermittedSet::new(vec![binding(1)]).expect("набор");
    assert!(set.contains(enrollment(1, 1)));
    assert!(!set.contains(enrollment(1, 2)));
}

#[test]
fn scoped_digest_covers_binding_and_format_not_only_registration() {
    let original = binding(1);
    let expected = PermittedSet::new(vec![original.clone()])
        .unwrap()
        .scoped_digest();
    let mut changed = original.clone();
    changed.provider_ref.push(9);
    assert_ne!(
        expected,
        PermittedSet::new(vec![changed]).unwrap().scoped_digest()
    );
    let mut changed = original;
    changed.format_id = None;
    assert_ne!(
        expected,
        PermittedSet::new(vec![changed]).unwrap().scoped_digest()
    );
}

#[test]
fn vault_binding_does_not_carry_a_foreign_reference() {
    let vault = EnrollmentBinding {
        enrollment: enrollment(3, 1),
        kind: BindingKind::ScopedVault,
        provider_ref: vec![1, 2, 3],
        format_id: None,
        format_version: 0,
    };
    assert_eq!(vault.validate(), Err(BindingError::KindMismatch));
}
