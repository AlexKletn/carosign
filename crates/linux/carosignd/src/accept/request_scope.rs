use super::{AuthenticatorId, OperationId, SubjectRef, SubjectSelector};

/// Area: `authority ‖ subject ‖ authenticator`48 bytes.
pub fn scope_of(
    subject: SubjectRef,
    authenticator: AuthenticatorId,
) -> [u8; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN] {
    let mut scope = [0u8; carosign_persistence::limits::SCOPE_LOCK_KEY_LEN];
    scope[..carosign_persistence::limits::SUBJECT_CANONICAL_LEN]
        .copy_from_slice(&subject.to_canonical());
    scope[carosign_persistence::limits::SUBJECT_CANONICAL_LEN..]
        .copy_from_slice(authenticator.as_bytes());
    scope
}

/// The number of the request operation (issued by the source, not the service).
pub fn operation_id(
    source: &dyn carosign_core::ids::IdSource,
) -> Result<OperationId, carosign_core::ids::IdUnavailable> {
    source.new_operation_id()
}

/// The choice of the subject from the client is data, not authority.
pub fn selector_is_current(selector: &SubjectSelector) -> bool {
    matches!(selector, SubjectSelector::Current)
}
