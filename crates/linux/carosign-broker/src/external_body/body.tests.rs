use super::*;

#[test]
fn body_round_trips() {
    let bytes = vec![7u8; 1024];
    let body = ExternalBody::for_bytes(&bytes).expect("тело");
    assert!(body.validate().is_ok());
    assert!(body.check_bytes(&bytes).is_ok());
}

#[test]
fn tampered_body_is_refused() {
    let bytes = vec![7u8; 1024];
    let body = ExternalBody::for_bytes(&bytes).expect("тело");
    let mut changed = bytes.clone();
    changed[10] = 8;
    assert_eq!(body.check_bytes(&changed), Err(BodyError::DigestMismatch));
}

#[test]
fn offset_and_size_are_checked() {
    let mut body = ExternalBody::for_bytes(&[1u8; 16]).expect("тело");
    body.offset = 4096;
    assert!(matches!(
        body.validate(),
        Err(BodyError::OffsetNotSupported { .. })
    ));
    let mut body = ExternalBody::for_bytes(&[1u8; 16]).expect("тело");
    body.file_size = 32;
    assert!(matches!(
        body.validate(),
        Err(BodyError::SizeMismatch { .. })
    ));
}

#[test]
fn typed_payload_can_accompany_external_body() {
    let body = ExternalBody::for_bytes(&[1u8; 16]).expect("тело");
    assert!(check_exclusive(16, Some(body)).is_ok());
    assert!(check_exclusive(16, None).is_ok());
    assert!(check_exclusive(PAYLOAD_MAX + 1, Some(body)).is_err());
}

#[test]
fn fd_count_must_match() {
    let body = ExternalBody::for_bytes(&[1u8; 16]).expect("тело");
    assert!(check_fd_count(Some(body), 1).is_ok());
    assert!(matches!(
        check_fd_count(Some(body), 2),
        Err(BodyError::WrongFdCount { actual: 2 })
    ));
    assert!(matches!(
        check_fd_count(None, 1),
        Err(BodyError::WrongFdCount { actual: 1 })
    ));
}

#[test]
fn oversized_body_is_refused() {
    assert!(matches!(
        ExternalBody::for_bytes(&vec![0u8; BODY_MAX + 1]),
        Err(BodyError::BadLength { .. })
    ));
}
