use super::*;

#[test]
fn unknown_wire_value_is_an_error() {
    // Invariant 5: the garbage in the enumeration is an error, not a default value.
    assert!(Modality::from_wire(0).is_err());
    assert!(Modality::from_wire(5).is_err());
    assert!(Modality::from_wire(-1).is_err());
    assert_eq!(Modality::from_wire(1), Ok(Modality::Fingerprint));
}

#[test]
fn wire_round_trip() {
    for (value, modality) in [
        (1, Modality::Fingerprint),
        (2, Modality::Face),
        (3, Modality::Iris),
        (4, Modality::Voice),
    ] {
        assert_eq!(modality.to_wire(), value);
        assert_eq!(Modality::from_wire(value), Ok(modality));
    }
}

#[test]
fn mutations_are_enroll_and_remove() {
    assert!(!OperationKind::Verify.is_mutation());
    assert!(OperationKind::Enroll.is_mutation());
    assert!(OperationKind::Remove.is_mutation());
}
