use super::{AcceptError, Candidate, MetadataStore, SelectionError, Table};

pub(super) fn read_candidates<S: MetadataStore>(
    store: &S,
    scope: &[u8; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN],
) -> Result<Vec<Candidate>, AcceptError> {
    Ok(read_enrollment_records(store, scope)?
        .iter()
        .map(Candidate::from_record)
        .collect())
}

/// A snapshot of the records and index of one area with a check of their interconnection.
pub(crate) fn read_enrollment_records<S: MetadataStore>(
    store: &S,
    scope: &[u8; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN],
) -> Result<Vec<carosign_persistence::enrollment_codec::EnrollmentRecord>, AcceptError> {
    let records = {
        // The lock, index and entries are read from a single image, without waiting.
        let tx = store
            .begin_read()
            .map_err(|_| AcceptError::IdentityUnavailable)?;
        let decode = |key: &[u8], bytes: &[u8]| {
            carosign_persistence::mutation_store::decode_enrollment_in_snapshot(&*tx, key, bytes)
                .map_err(|_| AcceptError::IdentityUnavailable)
        };
        if tx
            .get(Table::MutationScopeLock, scope)
            .map_err(|_| AcceptError::IdentityUnavailable)?
            .is_some()
        {
            return Err(AcceptError::Scope(SelectionError::PendingReconciliation));
        }
        let index = tx
            .scan_prefix(Table::EnrollmentIndex, scope)
            .map_err(|_| AcceptError::IdentityUnavailable)?;
        if index.len() > carosign_protocol::limits::ENROLLMENTS_PER_RESPONSE_MAX {
            return Err(AcceptError::IdentityUnavailable);
        }
        let mut records = Vec::with_capacity(index.len());
        for (key, value) in index {
            if key.len() != carosign_persistence::limits::ENROLLMENT_INDEX_KEY_LEN
                || value.len() != carosign_persistence::limits::ID_KEY_LEN
                || key[carosign_persistence::limits::SCOPE_LOCK_KEY_LEN..] != value[..]
            {
                return Err(AcceptError::IdentityUnavailable);
            }
            let bytes = tx
                .get(Table::Enrollments, &value)
                .map_err(|_| AcceptError::IdentityUnavailable)?
                .ok_or(AcceptError::IdentityUnavailable)?;
            let record = decode(&value, &bytes)?;
            if record.enrollment.id.as_bytes().as_slice() != value.as_slice()
                || record.subject.to_canonical().as_slice()
                    != &scope[..carosign_persistence::limits::SUBJECT_CANONICAL_LEN]
                || record.enrollment.authenticator.as_bytes().as_slice()
                    != &scope[carosign_persistence::limits::SUBJECT_CANONICAL_LEN..]
            {
                return Err(AcceptError::IdentityUnavailable);
            }
            records.push(record);
        }
        // Check feedback: a lost index should not hide the registration or give an incomplete
        // set for verification.
        for (key, bytes) in tx
            .scan_prefix(Table::Enrollments, &[])
            .map_err(|_| AcceptError::IdentityUnavailable)?
        {
            let record = decode(&key, &bytes)?;
            if key.as_slice() != record.enrollment.id.as_bytes() {
                return Err(AcceptError::IdentityUnavailable);
            }
            if record.subject.to_canonical().as_slice()
                == &scope[..carosign_persistence::limits::SUBJECT_CANONICAL_LEN]
                && record.enrollment.authenticator.as_bytes().as_slice()
                    == &scope[carosign_persistence::limits::SUBJECT_CANONICAL_LEN..]
                && !records
                    .iter()
                    .any(|entry| entry.enrollment == record.enrollment)
            {
                return Err(AcceptError::IdentityUnavailable);
            }
        }
        records
    };
    Ok(records)
}
