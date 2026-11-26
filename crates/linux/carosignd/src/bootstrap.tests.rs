use super::*;

#[test]
fn missing_store_is_not_silently_created() {
    // ⚠️ There is no base – a refusal, not “create an empty”: an empty base means “no
    // registrations”, and these are different statements.
    let path = "/nonexistent/definitely/metadata.redb";
    assert!(matches!(recover(path), Err(BootstrapError::Store(_))));
}

#[test]
fn coordinator_refuses_unknown_or_incomplete_scope_snapshot() {
    use carosign_core::identity::SubjectRef;
    use carosign_core::ids::{AuthenticatorId, AuthorityId, SubjectId};
    let mutation = MutationId::from_bytes([1; 16]).unwrap();
    let entry = BlockedMutationScope {
        mutation,
        subject: SubjectRef {
            authority: AuthorityId::from_bytes([2; 16]).unwrap(),
            subject: SubjectId::from_bytes([3; 16]).unwrap(),
        },
        authenticator: AuthenticatorId::from_bytes([4; 16]).unwrap(),
    };
    let mut recovered = Recovered {
        schema: SchemaStatus::Unverified,
        blocked: vec![],
        blocked_scopes: vec![],
        needs_decision: vec![],
    };
    assert!(recovered.coordinator().is_err());
    recovered.schema = SchemaStatus::V3;
    recovered.blocked.push(mutation);
    assert!(recovered.coordinator().is_err());
    recovered.blocked_scopes.push(entry);
    assert!(recovered.coordinator().is_ok());
    recovered.schema = SchemaStatus::V4;
    assert!(recovered.coordinator().is_ok());
    recovered.blocked.push(mutation);
    assert!(recovered.coordinator().is_err());
}
