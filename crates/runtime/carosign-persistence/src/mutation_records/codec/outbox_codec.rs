use super::{
    CommandId, EffectOutboxRecord, EffectPhase, Message, MutationId, MutationPhase, MutationRecord,
    OperationId, StorageError, corrupt, sha256, validate_v3, wire,
};

/// Encodes the recording of the v3 queue without executing or resending the effect.
pub fn encode_outbox(record: &EffectOutboxRecord) -> Result<Vec<u8>, StorageError> {
    if record.key.transition_index.value() == 0 {
        return Err(corrupt());
    }
    let value = wire::StoredValue {
        schema_version: 3,
        value: Some(wire::stored_value::Value::Outbox(
            wire::EffectOutboxRecord {
                operation_id: record.key.operation.as_bytes().to_vec(),
                transition_index: record.key.transition_index.value(),
                effect_position: record.key.position.value(),
                command_id: record
                    .command
                    .map(|id| id.as_bytes().to_vec())
                    .unwrap_or_default(),
                phase: match record.phase {
                    EffectPhase::Planned => 1,
                    EffectPhase::Armed => 2,
                    EffectPhase::Observed => 3,
                    EffectPhase::ReconciliationRequired => 4,
                },
                mutation_id: record
                    .mutation
                    .map(|id| id.as_bytes().to_vec())
                    .unwrap_or_default(),
            },
        )),
    };
    crate::enrollment_codec::envelope_with(&value.encode_to_vec(), sha256)
}

/// Disassembles the record and compares the mirror address with the redb key.
pub fn decode_outbox(key: &[u8], bytes: &[u8]) -> Result<EffectOutboxRecord, StorageError> {
    use carosign_engine::transition::{EffectKey, EffectPosition, TransitionIndex};
    let payload = crate::enrollment_codec::unenvelope_with(bytes, sha256).map_err(|_| corrupt())?;
    let value = wire::StoredValue::decode(payload.as_slice()).map_err(|_| corrupt())?;
    if value.schema_version != 3 {
        return Err(StorageError::UnsupportedSchema {
            found: value.schema_version,
        });
    }
    let Some(wire::stored_value::Value::Outbox(value)) = value.value else {
        return Err(corrupt());
    };
    if value.transition_index == 0 {
        return Err(corrupt());
    }
    let record = EffectOutboxRecord {
        key: EffectKey {
            operation: OperationId::try_from(value.operation_id.as_slice())
                .map_err(|_| corrupt())?,
            transition_index: TransitionIndex::new(value.transition_index),
            position: EffectPosition::new(value.effect_position),
        },
        command: if value.command_id.is_empty() {
            None
        } else {
            Some(CommandId::try_from(value.command_id.as_slice()).map_err(|_| corrupt())?)
        },
        phase: match value.phase {
            1 => EffectPhase::Planned,
            2 => EffectPhase::Armed,
            3 => EffectPhase::Observed,
            4 => EffectPhase::ReconciliationRequired,
            _ => return Err(corrupt()),
        },
        mutation: if value.mutation_id.is_empty() {
            None
        } else {
            Some(MutationId::try_from(value.mutation_id.as_slice()).map_err(|_| corrupt())?)
        },
    };
    if key != record.storage_key() {
        return Err(corrupt());
    }
    Ok(record)
}

/// Verifies the queue with the change record; does not allow re-sending.
pub fn validate_mutation_outbox(
    mutation: &MutationRecord,
    outbox: &EffectOutboxRecord,
) -> Result<(), StorageError> {
    validate_v3(mutation)?;
    if mutation.phase == MutationPhase::Prepared
        || outbox.key.transition_index.value() == 0
        || outbox.key.operation != mutation.operation
        || outbox.mutation != Some(mutation.mutation)
        || outbox.command.is_none()
        || outbox.command != mutation.command
        || outbox.phase == EffectPhase::Planned
    {
        return Err(corrupt());
    }
    // In these two phases, the recording of the command and its observation change with the
    // log in a single transaction.
    if matches!(mutation.phase, MutationPhase::DispatchArmed) && outbox.phase != EffectPhase::Armed
        || matches!(mutation.phase, MutationPhase::ProviderObserved)
            && outbox.phase != EffectPhase::Observed
    {
        return Err(corrupt());
    }
    Ok(())
}
