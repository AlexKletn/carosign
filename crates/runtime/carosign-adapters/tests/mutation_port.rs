use carosign_adapters::mutation::{JournalMutationPort, MutationIntent};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{
    AuthenticatorId, AuthorityId, CommandId, EnrollmentId, MutationId, OperationId, SubjectId,
};
use carosign_persistence::journal_store::StoreJournal;
use carosign_persistence::model::{MutationClosure, MutationPhase};
use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};
use carosign_persistence::test_support::MemoryStore;
use carosign_runtime::ports::MutationPort;

#[test]
fn mutation_port_never_invents_durable_receipts_or_hardware_identity() {
    let subject = SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let authenticator = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let mutation = MutationId::from_bytes([4; 16]).unwrap();
    let operation = OperationId::from_bytes([5; 16]).unwrap();
    let intent = MutationIntent {
        mutation,
        operation,
        subject,
        authenticator,
        expected_generation: None,
        enroll: true,
        plan: MutationPlan::build(
            Transition::EnrollNew,
            None,
            Some(EnrollmentId::from_bytes([8; 16]).unwrap()),
            1,
            Vec::new(),
            Scope {
                subject,
                authenticator,
            },
        )
        .unwrap(),
    };
    let journal = StoreJournal::legacy_fixture(MemoryStore::default());
    let uncertain_intent = intent.clone();
    let port = JournalMutationPort::new(
        &journal,
        intent,
        CommandId::from_bytes([6; 16]).unwrap(),
        MutationClosure::Completed,
    );
    let foreign = OperationId::from_bytes([9; 16]).unwrap();
    assert!(!futures_executor::block_on(port.persist_intent(foreign)).durable);
    assert!(journal.record(mutation).is_err());
    assert!(futures_executor::block_on(port.persist_intent(operation)).durable);
    assert_eq!(
        journal.record(mutation).unwrap().phase,
        MutationPhase::Prepared
    );
    assert_eq!(
        journal.record(mutation).unwrap().phase,
        MutationPhase::Prepared
    );
    assert!(!futures_executor::block_on(port.commit_registry(operation)).durable);
    assert!(!futures_executor::block_on(port.commit_reconciliation(operation)).durable);
    assert!(!futures_executor::block_on(port.mark_uncertain(foreign)).durable);
    assert!(!futures_executor::block_on(port.finalize(foreign)).durable);
    assert!(!futures_executor::block_on(port.finalize(operation)).durable);
    assert_eq!(
        journal.record(mutation).unwrap().phase,
        MutationPhase::Prepared
    );
    let uncertain_port = JournalMutationPort::new(
        &journal,
        uncertain_intent,
        CommandId::from_bytes([6; 16]).unwrap(),
        MutationClosure::NeedsReconciliation,
    );
    assert!(futures_executor::block_on(uncertain_port.mark_uncertain(operation)).durable);
    assert!(!futures_executor::block_on(uncertain_port.finalize(operation)).durable);
    assert_eq!(
        journal.record(mutation).unwrap().phase,
        MutationPhase::NeedsReconciliation
    );
}
