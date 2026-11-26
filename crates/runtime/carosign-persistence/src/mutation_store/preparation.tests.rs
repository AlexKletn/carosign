use super::{stage_prepared_v4, stage_prepared_v5};
use crate::idempotency::Reservation;
use crate::journal::PrepareMutation;
use crate::model::IdempotencyRecord;
use crate::mutation_plan::{MutationPlan, Scope, Transition};
use crate::mutation_store::{stage_pre_dispatch_no_change_v4, stage_pre_dispatch_no_change_v5};
use crate::redb::backend::InitialFenceStore;
use crate::store::{Table, WriteTransaction};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, EnrollmentId, IdempotencyKey, MutationId, OperationId, SubjectId,
};

#[test]
fn guarded_v5_preparation_is_durable_and_v4_entry_refuses_it() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "carosign-v5-prepare-{}-{nonce}",
        std::process::id()
    ));
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let input = PrepareMutation {
        mutation: MutationId::from_bytes([4; 16]).unwrap(),
        operation: OperationId::from_bytes([5; 16]).unwrap(),
        subject,
        authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([6; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject,
                authenticator,
            },
        )
        .unwrap(),
    };
    let request = IdempotencyRecord {
        caller_digest: [1; 32],
        key: IdempotencyKey::from_bytes([2; 16]).unwrap(),
        request_digest: [3; 32],
        operation: input.operation,
        mutation: Some(input.mutation),
    };
    let store = InitialFenceStore::create_new(&path).unwrap();
    let mut tx = store.begin_guarded_write().unwrap();
    assert!(stage_prepared_v4(&mut tx, &input, &request).is_err());
    assert_eq!(
        stage_prepared_v5(&mut tx, &input, &request),
        Ok(Reservation::Staged)
    );
    Box::new(tx).commit_durable().unwrap();
    drop(store);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    let mut tx = reopened.begin_guarded_write().unwrap();
    assert_eq!(tx.scan_prefix(Table::Mutations, &[]).unwrap().len(), 1);
    let other = OperationId::from_bytes([9; 16]).unwrap();
    assert!(stage_pre_dispatch_no_change_v5(&mut tx, input.mutation, other).is_err());
    Box::new(tx).abort();
    let mut tx = reopened.begin_guarded_write().unwrap();
    assert!(stage_pre_dispatch_no_change_v4(&mut tx, input.mutation, input.operation).is_err());
    stage_pre_dispatch_no_change_v5(&mut tx, input.mutation, input.operation).unwrap();
    Box::new(tx).commit_durable().unwrap();
    drop(reopened);
    let reopened = InitialFenceStore::open_guarded(&path).unwrap();
    let mut tx = reopened.begin_guarded_write().unwrap();
    assert!(stage_pre_dispatch_no_change_v5(&mut tx, input.mutation, input.operation).is_err());
    assert_eq!(
        tx.scan_prefix(Table::MutationScopeLock, &[]).unwrap().len(),
        1
    );
    assert!(tx.scan_prefix(Table::EffectOutbox, &[]).unwrap().is_empty());
    Box::new(tx).abort();
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}
