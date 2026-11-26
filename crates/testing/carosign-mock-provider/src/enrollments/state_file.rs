use carosign_protocol::validation::require_id;

use super::{MockEnrollments, mock_record::MockRecord, mock_state::MockState};

pub(crate) fn load_mock_state(path: &std::path::Path) -> Result<MockEnrollments, String> {
    use prost::Message;
    let bytes = match std::fs::read(path) {
        Ok(bytes) if bytes.len() <= 1024 * 1024 => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MockEnrollments::new());
        }
        _ => return Err("cannot read mock state".into()),
    };
    let state = MockState::decode(bytes.as_slice()).map_err(|_| "invalid mock state")?;
    if state.records.len() > 256 {
        return Err("too many mock records".into());
    }
    let mut result = MockEnrollments::new();
    for record in state.records {
        let subject = record.subject.ok_or("missing mock subject")?;
        let enrollment = record.enrollment.ok_or("missing mock enrollment")?;
        if require_id(&subject.authority_id, "mock enrollment ID").is_err()
            || require_id(&subject.subject_id, "mock enrollment ID").is_err()
            || require_id(&enrollment.enrollment_id, "mock enrollment ID").is_err()
            || require_id(&enrollment.authenticator_id, "mock enrollment ID").is_err()
            || enrollment.generation == 0
            || record.opaque.is_empty()
            || record.opaque.len() > 4096
            || result
                .insert(
                    enrollment.enrollment_id.clone(),
                    (subject, enrollment, record.opaque),
                )
                .is_some()
        {
            return Err("invalid mock record".into());
        }
    }
    Ok(result)
}

pub(crate) fn save_mock_state(
    path: &std::path::Path,
    records: &MockEnrollments,
) -> Result<(), String> {
    use prost::Message;
    let state = MockState {
        records: records
            .values()
            .map(|(subject, enrollment, opaque)| MockRecord {
                subject: Some(subject.clone()),
                enrollment: Some(enrollment.clone()),
                opaque: opaque.clone(),
            })
            .collect(),
    };
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temp, state.encode_to_vec()).map_err(|_| "cannot write mock state")?;
    std::fs::rename(&temp, path).map_err(|_| "cannot publish mock state".to_owned())
}
