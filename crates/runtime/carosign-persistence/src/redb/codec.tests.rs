use super::*;

#[test]
fn unsupported_envelope_schema_is_not_silently_discarded() {
    let envelope = Envelope {
        schema: SCHEMA_VERSION + 1,
        payload: vec![1],
    };
    assert!(matches!(
        envelope.encode(),
        Err(StorageError::UnsupportedSchema { .. })
    ));
}

#[test]
fn envelope_round_trip() {
    let env = Envelope {
        schema: SCHEMA_VERSION,
        payload: b"payload".to_vec(),
    };
    let bytes = env.encode().expect("годится");
    let back = Envelope::decode(&bytes).expect("разбирается");
    assert_eq!(back.payload, b"payload");
}

#[test]
fn corrupted_payload_is_rejected() {
    let env = Envelope {
        schema: SCHEMA_VERSION,
        payload: b"payload".to_vec(),
    };
    let mut bytes = env.encode().expect("годится");
    bytes[9] ^= 0xff;
    assert!(matches!(
        Envelope::decode(&bytes),
        Err(StorageError::Corrupt { .. })
    ));
}

#[test]
fn wrong_length_is_rejected() {
    let env = Envelope {
        schema: SCHEMA_VERSION,
        payload: b"payload".to_vec(),
    };
    let bytes = env.encode().expect("годится");
    assert!(Envelope::decode(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn empty_payload_is_refused() {
    let env = Envelope {
        schema: SCHEMA_VERSION,
        payload: Vec::new(),
    };
    assert!(env.encode().is_err());
}

#[test]
fn keys_have_canonical_lengths() {
    let id = [7u8; 16];
    assert_eq!(outbox_key(&id, 3, 2).len(), 28);
    assert_eq!(template_key(&id, 1).len(), 24);
    assert_eq!(id_key(&id).len(), 16);
    // Order in the key of the queue: operation, then the number bytes.
    let a = outbox_key(&id, 1, 0);
    let b = outbox_key(&id, 2, 0);
    assert!(a < b);
}
