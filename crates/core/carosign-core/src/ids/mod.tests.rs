use super::*;

#[test]
fn zero_id_is_rejected() {
    // C-01: The 16-byte null identifier is rejected in the constructor.
    assert_eq!(AuthorityId::from_bytes([0u8; 16]), Err(InvalidId));
    assert_eq!(SubjectId::from_bytes([0u8; 16]), Err(InvalidId));
    assert_eq!(OperationId::from_bytes([0u8; 16]), Err(InvalidId));
    assert_eq!(EnrollmentId::from_bytes([0u8; 16]), Err(InvalidId));
    assert_eq!(ResourceId::from_bytes([0u8; 16]), Err(InvalidId));
}

#[test]
fn single_nonzero_byte_is_enough() {
    let mut bytes = [0u8; 16];
    bytes[15] = 1;
    let id = SubjectId::from_bytes(bytes).expect("ненулевой");
    assert_eq!(id.as_bytes(), &bytes);
}

#[test]
fn bytes_round_trip() {
    let bytes = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32,
        0x10,
    ];
    let id = AuthenticatorId::from_bytes(bytes).expect("валид");
    assert_eq!(id.as_bytes(), &bytes);
    assert_eq!(AuthenticatorId::try_from(&bytes[..]), Ok(id));
}

#[test]
fn wrong_length_is_rejected() {
    assert!(PackageId::try_from(&[1u8; 15][..]).is_err());
    assert!(PackageId::try_from(&[1u8; 17][..]).is_err());
    assert!(PackageId::try_from(&[0u8; 0][..]).is_err());
}

#[test]
fn debug_does_not_leak_identity() {
    // The lines in the journal are put without content: it is identity.
    let id = WorkerInstanceId::from_bytes([9u8; 16]).expect("валид");
    let text = format!("{id:?}");
    assert_eq!(text, "WorkerInstanceId(..)");
    let subject = SubjectId::from_bytes([0xabu8; 16]).expect("валид");
    assert_eq!(format!("{subject:?}"), "SubjectId(..)");
}
