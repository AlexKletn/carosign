use super::*;

#[test]
fn enrollment_index_requires_nonzero_scope_and_exact_unchanged_id() {
    let migration = [5; 16];
    let mut index = [1u8; 64];
    index[48..].fill(7);
    let source = [7u8; 16];
    let encoded_key = Key::encode(migration, TableKind::EnrollmentIndex, &index).unwrap();
    let key = Key::decode(&encoded_key).unwrap();
    let check = |source: &[u8], prepared: &[u8]| {
        let encoded = Value::encode(
            TableKind::EnrollmentIndex,
            crate::mutation_plan::sha256(source),
            prepared,
        )
        .unwrap();
        let value = Value::decode(&encoded, TableKind::EnrollmentIndex).unwrap();
        verify_supported_record(&key, &value, &migration, source)
    };
    assert_eq!(check(&source, &source), Ok(()));
    assert!(check(&source, &[8; 16]).is_err());
    assert!(check(&[8; 16], &[8; 16]).is_err());
    assert!(check(&[0; 16], &[0; 16]).is_err());
    let mut invalid = index;
    invalid[16..32].fill(0);
    let encoded = Key::encode(migration, TableKind::EnrollmentIndex, &invalid).unwrap();
    let key = Key::decode(&encoded).unwrap();
    let encoded = Value::encode(
        TableKind::EnrollmentIndex,
        crate::mutation_plan::sha256(&source),
        &source,
    )
    .unwrap();
    let value = Value::decode(&encoded, TableKind::EnrollmentIndex).unwrap();
    assert!(verify_supported_record(&key, &value, &migration, &source).is_err());
}

#[test]
fn every_kind_has_canonical_key_round_trip() {
    for (kind, key) in [
        (TableKind::Authenticators, vec![1; 16]),
        (TableKind::Enrollments, vec![1; 16]),
        (TableKind::EnrollmentIndex, vec![1; 64]),
        (TableKind::Mutations, vec![1; 16]),
        (TableKind::MutationScopeLock, vec![1; 48]),
        (TableKind::EffectOutbox, vec![1; 28]),
        (TableKind::Idempotency, vec![1; 48]),
        (TableKind::ProviderState, vec![1; 32]),
        (TableKind::Packages, b"org.example.worker".to_vec()),
        (TableKind::TemplateBlobs, vec![1; 24]),
    ] {
        let bytes = Key::encode([2; 16], kind, &key).unwrap();
        assert_eq!(
            Key::decode(&bytes).unwrap(),
            Key {
                migration: [2; 16],
                table: kind,
                source_key: &key
            }
        );
        let value = Value::encode(kind, [3; 32], b"value").unwrap();
        assert_eq!(
            Value::decode(&value, kind).unwrap(),
            Value {
                source_digest: [3; 32],
                prepared_value: b"value"
            }
        );
    }
}

#[test]
fn malformed_keys_and_values_never_decode() {
    let key = Key::encode([2; 16], TableKind::Enrollments, &[1; 16]).unwrap();
    for n in 0..key.len() {
        assert_eq!(Key::decode(&key[..n]), Err(InvalidStageRecord));
    }
    for (offset, replacement) in [(16, 0), (16, 11), (20, 15)] {
        let mut bad = key.clone();
        bad[offset] = replacement;
        assert_eq!(Key::decode(&bad), Err(InvalidStageRecord));
    }
    let mut zero = key.clone();
    zero[..16].fill(0);
    assert_eq!(Key::decode(&zero), Err(InvalidStageRecord));
    assert!(Key::encode([2; 16], TableKind::Packages, b"../bad").is_err());
    let value = Value::encode(TableKind::Enrollments, [1; 32], b"test").unwrap();
    for n in 0..value.len() {
        assert_eq!(
            Value::decode(&value[..n], TableKind::Enrollments),
            Err(InvalidStageRecord)
        );
    }
    let mut extra = value.clone();
    extra.push(0);
    assert_eq!(
        Value::decode(&extra, TableKind::Enrollments),
        Err(InvalidStageRecord)
    );
    assert!(Value::encode(TableKind::Enrollments, [1; 32], &[]).is_err());
    assert!(
        Value::encode(
            TableKind::Enrollments,
            [1; 32],
            &vec![0; limits::STORED_VALUE_MAX + 41]
        )
        .is_err()
    );
}

#[test]
fn wire_bytes_use_declared_fields_and_big_endian_lengths() {
    let key = Key::encode([0x12; 16], TableKind::Enrollments, &[0x34; 16]).unwrap();
    assert_eq!(&key[..16], &[0x12; 16]);
    assert_eq!(key[16], 2);
    assert_eq!(&key[17..21], &[0, 0, 0, 16]);
    assert_eq!(&key[21..], &[0x34; 16]);
    let value = Value::encode(TableKind::Enrollments, [0x56; 32], b"abc").unwrap();
    assert_eq!(&value[..32], &[0x56; 32]);
    assert_eq!(&value[32..36], &[0, 0, 0, 3]);
    assert_eq!(&value[36..], b"abc");
}

#[test]
fn conversion_is_rechecked_against_exact_source_and_identity() {
    use carosign_core::ids::{IdempotencyKey, OperationId};
    let record = crate::model::IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: OperationId::from_bytes([4; 16]).unwrap(),
        mutation: None,
    };
    let source_key = crate::redb::codec::idempotency_key(&record.caller_digest, &record.key);
    let source = crate::mutation_records::encode_idempotency(&record).unwrap();
    let prepared = crate::installation_records::prepare_v3_record(
        crate::installation_records::MigratableRecord::Idempotency,
        &source_key,
        &source,
    )
    .unwrap();
    let key_bytes = Key::encode([5; 16], TableKind::Idempotency, &source_key).unwrap();
    let value_bytes = Value::encode(
        TableKind::Idempotency,
        crate::mutation_plan::sha256(&source),
        &prepared,
    )
    .unwrap();
    let key = Key::decode(&key_bytes).unwrap();
    let value = Value::decode(&value_bytes, key.table).unwrap();
    assert_eq!(
        verify_supported_record(&key, &value, &[5; 16], &source),
        Ok(())
    );
    assert!(verify_supported_record(&key, &value, &[6; 16], &source).is_err());
    let mut wrong_source = source.clone();
    wrong_source[10] ^= 1;
    assert!(verify_supported_record(&key, &value, &[5; 16], &wrong_source).is_err());
    let mut wrong_value = value_bytes.clone();
    *wrong_value.last_mut().unwrap() ^= 1;
    assert!(
        verify_supported_record(
            &key,
            &Value::decode(&wrong_value, key.table).unwrap(),
            &[5; 16],
            &source
        )
        .is_err()
    );
    let unsupported = Key {
        table: TableKind::MutationScopeLock,
        ..key
    };
    assert!(verify_supported_record(&unsupported, &value, &[5; 16], &source).is_err());
}
