use super::{
    IdempotencyKey, IdempotencyRecord, Message, MutationId, OperationId, StorageError, corrupt,
    sha256, wire,
};

/// Encodes the idempotency record v3 with SHA-256.
pub fn encode_idempotency(record: &IdempotencyRecord) -> Result<Vec<u8>, StorageError> {
    let stored = wire::StoredValue {
        schema_version: 3,
        value: Some(wire::stored_value::Value::Idempotency(
            wire::IdempotencyRecord {
                caller_binding_digest: record.caller_digest.to_vec(),
                idempotency_key: record.key.as_bytes().to_vec(),
                normalized_request_digest: record.request_digest.to_vec(),
                operation_id: record.operation.as_bytes().to_vec(),
                mutation_id: record
                    .mutation
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default(),
            },
        )),
    };
    crate::enrollment_codec::envelope_with(&stored.encode_to_vec(), sha256)
}

/// Reads the record of idempotence and compares its fields with the table key.
pub fn decode_idempotency(key: &[u8], bytes: &[u8]) -> Result<IdempotencyRecord, StorageError> {
    let payload = crate::enrollment_codec::unenvelope_with(bytes, sha256).map_err(|_| corrupt())?;
    let stored = wire::StoredValue::decode(payload.as_slice()).map_err(|_| corrupt())?;
    if stored.schema_version != 3 {
        return Err(StorageError::UnsupportedSchema {
            found: stored.schema_version,
        });
    }
    let Some(wire::stored_value::Value::Idempotency(value)) = stored.value else {
        return Err(corrupt());
    };
    let record = IdempotencyRecord {
        caller_digest: value
            .caller_binding_digest
            .try_into()
            .map_err(|_| corrupt())?,
        key: IdempotencyKey::try_from(value.idempotency_key.as_slice()).map_err(|_| corrupt())?,
        request_digest: value
            .normalized_request_digest
            .try_into()
            .map_err(|_| corrupt())?,
        operation: OperationId::try_from(value.operation_id.as_slice()).map_err(|_| corrupt())?,
        mutation: if value.mutation_id.is_empty() {
            None
        } else {
            Some(MutationId::try_from(value.mutation_id.as_slice()).map_err(|_| corrupt())?)
        },
    };
    if key != crate::redb::codec::idempotency_key(&record.caller_digest, &record.key) {
        return Err(corrupt());
    }
    Ok(record)
}
