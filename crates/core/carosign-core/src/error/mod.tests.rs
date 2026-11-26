use super::*;

#[test]
fn domain_error_says_what_is_wrong() {
    let e = DomainError::UnknownEnumValue(42);
    assert_eq!(e.to_string(), "значение перечисления 42 не поддерживается");
    let v = DomainError::UnsupportedVersion {
        found: 2,
        supported: 1,
    };
    assert_eq!(
        v.to_string(),
        "версия 2 не поддерживается (поддерживается 1)"
    );
}

#[test]
fn limits_are_bounded_and_sane() {
    // Verification is that the set didn't move when it was edited. Numbers from document 13.
    assert_eq!(limits::CONTROL_FRAME_MAX, 65_536);
    assert_eq!(limits::FDS_PER_FRAME_MAX, 8);
    assert_eq!(limits::OPAQUE_PROVIDER_REF_MAX, 4_096);
    assert_eq!(limits::ENROLLMENT_LABEL_MAX, 64);
    assert_eq!(limits::TEMPLATE_BLOB_MAX, 8_388_608);
    assert_eq!(limits::METADATA_VALUE_MAX, 65_536);
    assert_eq!(limits::BROKER_PAYLOAD_MAX, 32_768);
}
