use super::{
    AuthenticatorId, AuthorityId, EnrollmentGeneration, EnrollmentId, EnrollmentRecord,
    EnrollmentRef, EnrollmentStatus, Message, OpaqueProviderRef, RecordError, SCHEMA_VERSION,
    StorageError, SubjectId, SubjectRef, envelope, envelope_with, limits, proto, unenvelope,
    unenvelope_with,
};

/// Dealing with the value of the registry.
pub fn decode(bytes: &[u8]) -> Result<EnrollmentRecord, RecordError> {
    let payload = unenvelope(bytes)?;
    decode_payload(&payload)
}

/// Disassembles the v3 record with the SHA-256 regulatory shell.
///
/// It does not accept a temporary shell and does not perform migration.
pub fn decode_v3(bytes: &[u8]) -> Result<EnrollmentRecord, RecordError> {
    let payload = unenvelope_with(bytes, crate::mutation_plan::sha256)?;
    decode_payload(&payload)
}

fn decode_payload(payload: &[u8]) -> Result<EnrollmentRecord, RecordError> {
    let value = proto::StoredValue::decode(payload).map_err(|_| {
        RecordError::Envelope(StorageError::Corrupt {
            what: "StoredValue не разбирается",
        })
    })?;
    if value.schema_version != SCHEMA_VERSION {
        return Err(RecordError::UnsupportedSchema {
            found: value.schema_version,
        });
    }
    let Some(proto::stored_value::Value::Enrollment(record)) = value.value else {
        return Err(RecordError::NotAnEnrollment);
    };

    let enrollment = record.enrollment.ok_or(RecordError::BadField {
        field: "enrollment",
    })?;
    let subject = record
        .subject
        .ok_or(RecordError::BadField { field: "subject" })?;

    let authority =
        AuthorityId::try_from(&subject.authority_id[..]).map_err(|_| RecordError::BadField {
            field: "subject.authority_id",
        })?;
    let subject_id =
        SubjectId::try_from(&subject.subject_id[..]).map_err(|_| RecordError::BadField {
            field: "subject.subject_id",
        })?;
    let enrollment_id = EnrollmentId::try_from(&enrollment.enrollment_id[..]).map_err(|_| {
        RecordError::BadField {
            field: "enrollment.enrollment_id",
        }
    })?;
    let authenticator =
        AuthenticatorId::try_from(&enrollment.authenticator_id[..]).map_err(|_| {
            RecordError::BadField {
                field: "enrollment.authenticator_id",
            }
        })?;
    let generation = EnrollmentGeneration::new(enrollment.generation)
        .map_err(|_| RecordError::ZeroGeneration)?;

    if record.label.len() > limits::LABEL_MAX
        || record.display_label.as_ref().is_some_and(|label| label.len() > limits::LABEL_MAX)
    {
        return Err(RecordError::BadField { field: "label" });
    }
    if record.template_format_id.len() > 128
        || record.template_format_id.chars().any(char::is_control)
    {
        return Err(RecordError::BadField {
            field: "template_format_id",
        });
    }
    let provider_ref =
        OpaqueProviderRef::new(record.provider_opaque_ref).map_err(|_| RecordError::BadField {
            field: "provider_opaque_ref",
        })?;

    let binding_kind = crate::mutation_plan::StoredBindingKind::from_wire(record.binding_kind)
        .map_err(|_| RecordError::BadField {
            field: "binding_kind",
        })?;
    if binding_kind == crate::mutation_plan::StoredBindingKind::ScopedVault
        && !provider_ref.as_bytes().is_empty()
    {
        return Err(RecordError::BadField {
            field: "provider_opaque_ref",
        });
    }
    Ok(EnrollmentRecord {
        subject: SubjectRef {
            authority,
            subject: subject_id,
        },
        enrollment: EnrollmentRef {
            id: enrollment_id,
            authenticator,
            generation,
        },
        label: record.label,
        display_label: record.display_label,
        status: EnrollmentStatus::from_wire(record.status)?,
        provider_ref,
        format_id: record.template_format_id,
        format_version: record.template_format_version,
        binding_kind,
    })
}

/// Gathers the value of the registry.
pub fn encode(record: &EnrollmentRecord) -> Result<Vec<u8>, RecordError> {
    let payload = encode_payload(record)?;
    envelope(&payload).map_err(RecordError::Envelope)
}

/// Encodes the v3 record with the SHA-256 regulatory shell.
///
/// Entering an existing database requires a check of the scheme and a separate migration.
pub fn encode_v3(record: &EnrollmentRecord) -> Result<Vec<u8>, RecordError> {
    let payload = encode_payload(record)?;
    envelope_with(&payload, crate::mutation_plan::sha256).map_err(RecordError::Envelope)
}

fn encode_payload(record: &EnrollmentRecord) -> Result<Vec<u8>, RecordError> {
    if record.label.len() > limits::LABEL_MAX
        || record.display_label.as_ref().is_some_and(|label| label.len() > limits::LABEL_MAX)
    {
        return Err(RecordError::BadField { field: "label" });
    }
    if record.format_id.len() > 128 || record.format_id.chars().any(char::is_control) {
        return Err(RecordError::BadField {
            field: "template_format_id",
        });
    }
    if record.binding_kind == crate::mutation_plan::StoredBindingKind::ScopedVault
        && !record.provider_ref.as_bytes().is_empty()
    {
        return Err(RecordError::BadField {
            field: "provider_opaque_ref",
        });
    }
    let value = proto::StoredValue {
        schema_version: SCHEMA_VERSION,
        value: Some(proto::stored_value::Value::Enrollment(
            proto::EnrollmentRecord {
                enrollment: Some(proto::EnrollmentRef {
                    enrollment_id: record.enrollment.id.as_bytes().to_vec(),
                    authenticator_id: record.enrollment.authenticator.as_bytes().to_vec(),
                    generation: record.enrollment.generation.value(),
                }),
                subject: Some(proto::SubjectRef {
                    authority_id: record.subject.authority.as_bytes().to_vec(),
                    subject_id: record.subject.subject.as_bytes().to_vec(),
                }),
                label: record.label.clone(),
                display_label: record.display_label.clone(),
                status: record.status.wire(),
                provider_opaque_ref: record.provider_ref.as_bytes().to_vec(),
                template_format_id: record.format_id.clone(),
                template_format_version: record.format_version,
                binding_kind: record.binding_kind.wire(),
            },
        )),
    };
    Ok(value.encode_to_vec())
}
