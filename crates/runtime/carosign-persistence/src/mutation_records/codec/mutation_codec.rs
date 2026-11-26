use super::{
    AuthenticatorId, AuthorityId, BindingSnapshot, CommandId, DeviceEpoch, EnrollmentGeneration,
    EnrollmentId, EnrollmentRef, Message, MutationId, MutationPhase, MutationPlan, MutationRecord,
    MutationTerminalOutcome, OpaqueProviderRef, OperationId, Scope, StorageError,
    StoredBindingKind, SubjectId, SubjectRef, Transition, WorkerInstanceId, WriteTransaction,
    common, corrupt, sha256, wire,
};

fn reference(value: EnrollmentRef) -> common::EnrollmentRef {
    common::EnrollmentRef {
        enrollment_id: value.id.as_bytes().to_vec(),
        authenticator_id: value.authenticator.as_bytes().to_vec(),
        generation: value.generation.value(),
    }
}

fn parse_reference(value: common::EnrollmentRef) -> Result<EnrollmentRef, StorageError> {
    Ok(EnrollmentRef {
        id: EnrollmentId::try_from(value.enrollment_id.as_slice()).map_err(|_| corrupt())?,
        authenticator: AuthenticatorId::try_from(value.authenticator_id.as_slice())
            .map_err(|_| corrupt())?,
        generation: EnrollmentGeneration::new(value.generation).map_err(|_| corrupt())?,
    })
}

fn subject(value: SubjectRef) -> common::SubjectRef {
    common::SubjectRef {
        authority_id: value.authority.as_bytes().to_vec(),
        subject_id: value.subject.as_bytes().to_vec(),
    }
}

fn parse_subject(value: common::SubjectRef) -> Result<SubjectRef, StorageError> {
    Ok(SubjectRef {
        authority: AuthorityId::try_from(value.authority_id.as_slice()).map_err(|_| corrupt())?,
        subject: SubjectId::try_from(value.subject_id.as_slice()).map_err(|_| corrupt())?,
    })
}

fn wire_plan(plan: &MutationPlan) -> wire::MutationPlanRecord {
    wire::MutationPlanRecord {
        transition: plan.transition.wire(),
        new_enrollment_id: plan
            .new_enrollment
            .map(|id| id.as_bytes().to_vec())
            .unwrap_or_default(),
        new_generation: plan.new_generation,
        pre_bindings: plan
            .pre_bindings
            .iter()
            .map(|entry| wire::BindingSnapshot {
                enrollment: Some(reference(entry.enrollment)),
                subject: Some(subject(entry.subject)),
                provider_ref_digest: entry.provider_ref_digest.to_vec(),
                format_id: entry.format_id.clone(),
                format_version: entry.format_version,
                binding_kind: entry.kind.wire(),
            })
            .collect(),
        affected_scope_digest: plan.scope_digest.to_vec(),
        old_target: plan.old_target.map(reference),
        requested_label: plan.requested_label.clone(),
    }
}

fn parse_plan(plan: wire::MutationPlanRecord, scope: Scope) -> Result<MutationPlan, StorageError> {
    if plan.pre_bindings.len() > crate::mutation_plan::SNAPSHOTS_MAX {
        return Err(corrupt());
    }
    let mut pre_bindings = Vec::with_capacity(plan.pre_bindings.len());
    for entry in plan.pre_bindings {
        pre_bindings.push(BindingSnapshot {
            enrollment: parse_reference(entry.enrollment.ok_or_else(corrupt)?)?,
            subject: parse_subject(entry.subject.ok_or_else(corrupt)?)?,
            kind: StoredBindingKind::from_wire(entry.binding_kind).map_err(|_| corrupt())?,
            provider_ref_digest: entry
                .provider_ref_digest
                .try_into()
                .map_err(|_| corrupt())?,
            format_id: entry.format_id,
            format_version: entry.format_version,
        });
    }
    let result = MutationPlan {
        transition: Transition::from_wire(plan.transition).map_err(|_| corrupt())?,
        new_enrollment: if plan.new_enrollment_id.is_empty() {
            None
        } else {
            Some(EnrollmentId::try_from(plan.new_enrollment_id.as_slice()).map_err(|_| corrupt())?)
        },
        new_generation: plan.new_generation,
        old_target: plan.old_target.map(parse_reference).transpose()?,
        requested_label: plan.requested_label,
        pre_bindings,
        scope_digest: plan
            .affected_scope_digest
            .try_into()
            .map_err(|_| corrupt())?,
    };
    result.validate(scope).map_err(|_| corrupt())?;
    Ok(result)
}

pub(super) fn validate_v3(record: &MutationRecord) -> Result<(), StorageError> {
    record.validate().map_err(|_| corrupt())?;
    let plan = record.plan.as_ref().ok_or_else(corrupt)?;
    plan.validate(Scope {
        subject: record.subject,
        authenticator: record.authenticator,
    })
    .map_err(|_| corrupt())?;
    let no_dispatch = record.worker.is_none() && record.epoch.is_none() && record.command.is_none();
    let complete_fence =
        record.worker.is_some() && record.epoch.is_some() && record.command.is_some();
    let early_no_change = no_dispatch
        && (record.phase == MutationPhase::NoChange
            || record.phase == MutationPhase::Completed
                && record.terminal_outcome == Some(MutationTerminalOutcome::NoChange));
    if record.phase != MutationPhase::Prepared && !complete_fence && !early_no_change {
        return Err(corrupt());
    }
    if early_no_change
        && (record.provider_observation_digest.is_some() || record.recovery_ref.is_some())
    {
        return Err(corrupt());
    }
    if (record.phase == MutationPhase::Completed) != record.terminal_outcome.is_some() {
        return Err(corrupt());
    }
    if matches!(
        record.phase,
        MutationPhase::Prepared | MutationPhase::DispatchArmed
    ) && record.provider_observation_digest.is_some()
    {
        return Err(corrupt());
    }
    if record.phase == MutationPhase::ProviderObserved
        && record.provider_observation_digest.is_none()
    {
        return Err(corrupt());
    }
    Ok(())
}

/// Encodes a v3 entry; the mere fact of recording does not issue a sending ticket.
pub fn encode_mutation(record: &MutationRecord) -> Result<Vec<u8>, StorageError> {
    encode_mutation_version(record, 3)
}

pub(in super::super) fn encode_record_for_schema(
    _tx: &dyn WriteTransaction,
    record: &MutationRecord,
) -> Result<Vec<u8>, StorageError> {
    encode_mutation(record)
}

fn encode_mutation_version(record: &MutationRecord, schema: u32) -> Result<Vec<u8>, StorageError> {
    validate_v3(record)?;
    let plan = record.plan.as_ref().ok_or_else(corrupt)?;
    if schema != 3 {
        return Err(StorageError::UnsupportedSchema { found: schema });
    }
    let phase = match record.phase {
        MutationPhase::Prepared => 1,
        MutationPhase::DispatchArmed => 2,
        MutationPhase::ProviderObserved => 3,
        MutationPhase::RegistryCommitted => 4,
        MutationPhase::Completed => 5,
        MutationPhase::NeedsReconciliation => 6,
        MutationPhase::NoChange => 7,
    };
    if record
        .recovery_ref
        .as_ref()
        .is_some_and(|value| value.as_bytes().is_empty())
    {
        return Err(corrupt());
    }
    let value = wire::StoredValue {
        schema_version: schema,
        value: Some(wire::stored_value::Value::Mutation(wire::MutationRecord {
            mutation_id: record.mutation.as_bytes().to_vec(),
            operation_id: record.operation.as_bytes().to_vec(),
            subject: Some(subject(record.subject)),
            authenticator_id: record.authenticator.as_bytes().to_vec(),
            expected_generation: record.expected_generation.map_or(0, |value| value.value()),
            kind: match plan.transition {
                Transition::EnrollNew | Transition::EnrollReplace => 1,
                Transition::Remove => 2,
            },
            phase,
            worker_instance_id: record
                .worker
                .map(|value| value.as_bytes().to_vec())
                .unwrap_or_default(),
            device_epoch: record.epoch.map_or(0, |value| value.value()),
            command_id: record
                .command
                .map(|value| value.as_bytes().to_vec())
                .unwrap_or_default(),
            recovery_ref: record
                .recovery_ref
                .as_ref()
                .map(|value| value.as_bytes().to_vec())
                .unwrap_or_default(),
            target_enrollment: plan.old_target.map(reference),
            provider_observation_digest: record
                .provider_observation_digest
                .map_or_else(Vec::new, |digest| digest.to_vec()),
            terminal_outcome: match record.terminal_outcome {
                None => 0,
                Some(MutationTerminalOutcome::Committed) => 1,
                Some(MutationTerminalOutcome::NoChange) => 2,
            },
            plan: Some(wire_plan(plan)),
        })),
    };
    crate::enrollment_codec::envelope_with(&value.encode_to_vec(), sha256)
}

/// Reads v3 without implicit migration or making a decision about hardware status.
pub fn decode_mutation(bytes: &[u8]) -> Result<MutationRecord, StorageError> {
    let payload = crate::enrollment_codec::unenvelope_with(bytes, sha256).map_err(|_| corrupt())?;
    let value = wire::StoredValue::decode(payload.as_slice()).map_err(|_| corrupt())?;
    let schema = value.schema_version;
    if schema != 3 {
        return Err(StorageError::UnsupportedSchema {
            found: value.schema_version,
        });
    }
    let Some(wire::stored_value::Value::Mutation(value)) = value.value else {
        return Err(corrupt());
    };
    let phase = match value.phase {
        1 => MutationPhase::Prepared,
        2 => MutationPhase::DispatchArmed,
        3 => MutationPhase::ProviderObserved,
        4 => MutationPhase::RegistryCommitted,
        5 => MutationPhase::Completed,
        6 => MutationPhase::NeedsReconciliation,
        7 => MutationPhase::NoChange,
        _ => return Err(corrupt()),
    };
    let subject = parse_subject(value.subject.ok_or_else(corrupt)?)?;
    let authenticator =
        AuthenticatorId::try_from(value.authenticator_id.as_slice()).map_err(|_| corrupt())?;
    let plan = parse_plan(
        value.plan.ok_or_else(corrupt)?,
        Scope {
            subject,
            authenticator,
        },
    )?;
    if value.kind
        != match plan.transition {
            Transition::EnrollNew | Transition::EnrollReplace => 1,
            Transition::Remove => 2,
        }
        || value.target_enrollment.map(parse_reference).transpose()? != plan.old_target
    {
        return Err(corrupt());
    }
    let worker = if value.worker_instance_id.is_empty() {
        None
    } else {
        Some(
            WorkerInstanceId::try_from(value.worker_instance_id.as_slice())
                .map_err(|_| corrupt())?,
        )
    };
    let epoch = if value.device_epoch == 0 {
        None
    } else {
        Some(DeviceEpoch::new(value.device_epoch).map_err(|_| corrupt())?)
    };
    let command = if value.command_id.is_empty() {
        None
    } else {
        Some(CommandId::try_from(value.command_id.as_slice()).map_err(|_| corrupt())?)
    };
    let record = MutationRecord {
        mutation: MutationId::try_from(value.mutation_id.as_slice()).map_err(|_| corrupt())?,
        operation: OperationId::try_from(value.operation_id.as_slice()).map_err(|_| corrupt())?,
        subject,
        authenticator,
        expected_generation: if value.expected_generation == 0 {
            None
        } else {
            Some(EnrollmentGeneration::new(value.expected_generation).map_err(|_| corrupt())?)
        },
        phase,
        worker,
        epoch,
        command,
        recovery_ref: if value.recovery_ref.is_empty() {
            None
        } else {
            Some(OpaqueProviderRef::new(value.recovery_ref).map_err(|_| corrupt())?)
        },
        provider_observation_digest: if value.provider_observation_digest.is_empty() {
            None
        } else {
            Some(
                value
                    .provider_observation_digest
                    .try_into()
                    .map_err(|_| corrupt())?,
            )
        },
        terminal_outcome: match value.terminal_outcome {
            0 => None,
            1 => Some(MutationTerminalOutcome::Committed),
            2 => Some(MutationTerminalOutcome::NoChange),
            _ => return Err(corrupt()),
        },
        plan: Some(plan),
    };
    validate_v3(&record)?;
    Ok(record)
}
