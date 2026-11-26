use super::*;

fn record() -> EnrollmentRecord {
    EnrollmentRecord {
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([1u8; 16]).expect("валид"),
            subject: SubjectId::from_bytes([2u8; 16]).expect("валид"),
        },
        enrollment: EnrollmentRef {
            id: EnrollmentId::from_bytes([3u8; 16]).expect("валид"),
            authenticator: AuthenticatorId::from_bytes([4u8; 16]).expect("валид"),
            generation: EnrollmentGeneration::FIRST,
        },
        display_label: None,
        label: "палец".into(),
        status: EnrollmentStatus::Active,
        provider_ref: OpaqueProviderRef::new(vec![7, 7]).expect("годится"),
        format_id: "goodix-asp".into(),
        format_version: 1,
        binding_kind: crate::mutation_plan::StoredBindingKind::ProviderOpaque,
    }
}

#[test]
fn duplicate_stored_value_is_corruption_even_with_valid_checksum() {
    let valid = encode_v3(&record()).unwrap();
    let payload = unenvelope_with(&valid, crate::mutation_plan::sha256).unwrap();
    for extra in [
        &[0x5a, 0x00][..],
        &[0x62, 0x00][..],
        &[0x92, 0x01, 0x00][..],
    ] {
        let mut duplicate = payload.clone();
        duplicate.extend_from_slice(extra);
        let wrapped = envelope_with(&duplicate, crate::mutation_plan::sha256).unwrap();
        assert!(matches!(
            decode_v3(&wrapped),
            Err(RecordError::Envelope(StorageError::Corrupt { .. }))
        ));
    }
    assert!(decode_v3(&valid).is_ok());
}

#[test]
fn round_trip_through_the_envelope() {
    let original = record();
    let bytes = encode(&original).expect("кодируется");
    assert_eq!(decode(&bytes), Ok(original));
}

#[test]
fn v3_envelope_interoperates_with_authoritative_generated_schema() {
    use carosign_protocol::carosign::storage::v1 as wire;
    let expected = record();
    let bytes = encode_v3(&expected).unwrap();
    let len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let payload = &bytes[8..8 + len];
    assert_eq!(&bytes[8 + len..], &crate::mutation_plan::sha256(payload));
    let decoded = wire::StoredValue::decode(payload).unwrap();
    assert_eq!(decoded.schema_version, 3);
    let Some(wire::stored_value::Value::Enrollment(enrollment)) = decoded.value else {
        panic!("нет записи регистрации")
    };
    assert_eq!(
        enrollment.enrollment.unwrap().enrollment_id,
        expected.enrollment.id.as_bytes()
    );
    assert_eq!(
        enrollment.subject.unwrap().subject_id,
        expected.subject.subject.as_bytes()
    );
    assert_eq!(enrollment.binding_kind, expected.binding_kind.wire());
    assert_eq!(enrollment.label, expected.label);
    assert_eq!(decode_v3(&bytes), Ok(expected));
}

#[test]
fn v3_never_silently_accepts_legacy_or_corrupted_envelope() {
    let original = record();
    let legacy = encode(&original).unwrap();
    let correct = encode_v3(&original).unwrap();
    assert!(decode_v3(&legacy).is_err());
    assert!(decode(&correct).is_err());
    let mut damaged = correct.clone();
    damaged[10] ^= 1;
    assert!(decode_v3(&damaged).is_err());
    let mut truncated = correct;
    truncated.pop();
    assert!(decode_v3(&truncated).is_err());
}

#[test]
fn v3_requires_its_exact_version_even_with_valid_sha256() {
    let bytes = encode_v3(&record()).unwrap();
    let payload = unenvelope_with(&bytes, crate::mutation_plan::sha256).unwrap();
    let mut value = proto::StoredValue::decode(payload.as_slice()).unwrap();
    for version in [1, 2, 4] {
        value.schema_version = version;
        let bytes = envelope_with(&value.encode_to_vec(), crate::mutation_plan::sha256).unwrap();
        assert_eq!(
            decode_v3(&bytes),
            Err(RecordError::UnsupportedSchema { found: version })
        );
    }
}

#[test]
fn damaged_sum_is_refused() {
    let bytes = encode(&record()).expect("кодируется");
    let mut damaged = bytes.clone();
    damaged[9] ^= 0xff;
    assert!(matches!(
        decode(&damaged),
        Err(RecordError::Envelope(StorageError::Corrupt { .. }))
    ));
}

#[test]
fn version_one_needs_migration() {
    // Version 2 requires a clear migration to Version 3.
    let mut value = proto::StoredValue::decode(
        unenvelope(&encode(&record()).expect("кодируется"))
            .expect("оболочка")
            .as_slice(),
    )
    .expect("разбор");
    value.schema_version = 2;
    let payload = value.encode_to_vec();
    let bytes = envelope(&payload).expect("оболочка");
    assert_eq!(
        decode(&bytes),
        Err(RecordError::UnsupportedSchema { found: 2 })
    );
}

#[test]
fn zero_generation_is_refused() {
    let mut value = record();
    value.enrollment.generation = EnrollmentGeneration::new(1).expect("валид");
    let mut raw = proto::StoredValue::decode(
        unenvelope(&encode(&value).expect("кодируется"))
            .expect("оболочка")
            .as_slice(),
    )
    .expect("разбор");
    if let Some(proto::stored_value::Value::Enrollment(inner)) = &mut raw.value {
        inner.enrollment.as_mut().expect("есть").generation = 0;
    }
    let bytes = envelope(&raw.encode_to_vec()).expect("оболочка");
    assert_eq!(decode(&bytes), Err(RecordError::ZeroGeneration));
}

#[test]
fn not_an_enrollment_is_refused() {
    let raw = proto::StoredValue {
        schema_version: SCHEMA_VERSION,
        value: None,
    };
    let bytes = envelope(&raw.encode_to_vec()).expect("оболочка");
    assert_eq!(decode(&bytes), Err(RecordError::NotAnEnrollment));
}

#[test]
fn scoped_vault_cannot_contain_provider_reference() {
    let mut value = record();
    value.binding_kind = crate::mutation_plan::StoredBindingKind::ScopedVault;
    assert_eq!(
        encode(&value),
        Err(RecordError::BadField {
            field: "provider_opaque_ref"
        })
    );
    let mut raw =
        proto::StoredValue::decode(unenvelope(&encode(&record()).unwrap()).unwrap().as_slice())
            .unwrap();
    let Some(proto::stored_value::Value::Enrollment(inner)) = &mut raw.value else {
        unreachable!()
    };
    inner.binding_kind = crate::mutation_plan::StoredBindingKind::ScopedVault.wire();
    assert_eq!(
        decode(&envelope(&raw.encode_to_vec()).unwrap()),
        Err(RecordError::BadField {
            field: "provider_opaque_ref"
        })
    );
    value.provider_ref = OpaqueProviderRef::new(vec![]).unwrap();
    assert_eq!(decode(&encode(&value).unwrap()), Ok(value));
}

#[test]
fn format_id_limit_is_checked_before_writing() {
    let mut value = record();
    value.format_id = "я".repeat(64);
    assert_eq!(decode(&encode(&value).unwrap()), Ok(value.clone()));
    value.format_id.push('a');
    assert_eq!(
        encode(&value),
        Err(RecordError::BadField {
            field: "template_format_id"
        })
    );
}

#[test]
fn format_id_control_characters_are_rejected_on_write_and_read() {
    let mut value = record();
    value.format_id = "format\nname".into();
    let expected = RecordError::BadField {
        field: "template_format_id",
    };
    assert_eq!(encode_v3(&value), Err(expected.clone()));

    let valid = encode_v3(&record()).unwrap();
    let payload = unenvelope_with(&valid, crate::mutation_plan::sha256).unwrap();
    let mut raw = proto::StoredValue::decode(payload.as_slice()).unwrap();
    let Some(proto::stored_value::Value::Enrollment(inner)) = &mut raw.value else {
        unreachable!()
    };
    inner.template_format_id = "format\u{7f}name".into();
    let malformed = envelope_with(&raw.encode_to_vec(), crate::mutation_plan::sha256).unwrap();
    assert_eq!(decode_v3(&malformed), Err(expected));
}
