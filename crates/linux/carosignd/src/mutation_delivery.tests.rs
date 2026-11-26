use super::*;
use carosign_core::ids::{AuthenticatorId, AuthorityId, EnrollmentId, SubjectId};
use carosign_persistence::mutation_plan::{MutationPlan, Scope, Transition};

#[test]
fn refuses_unfinished_or_changed_scope_before_terminal() {
    let subject = carosign_core::identity::SubjectRef {
        authority: AuthorityId::from_bytes([1; 16]).unwrap(),
        subject: SubjectId::from_bytes([2; 16]).unwrap(),
    };
    let auth = AuthenticatorId::from_bytes([3; 16]).unwrap();
    let plan = MutationPlan::build(
        Transition::EnrollNew,
        None,
        Some(EnrollmentId::from_bytes([4; 16]).unwrap()),
        1,
        vec![],
        Scope {
            subject,
            authenticator: auth,
        },
    )
    .unwrap();
    let mut record = MutationRecord {
        mutation: MutationId::from_bytes([5; 16]).unwrap(),
        operation: OperationId::from_bytes([6; 16]).unwrap(),
        subject,
        authenticator: auth,
        expected_generation: Some(
            carosign_core::enrollment::EnrollmentGeneration::new(1).unwrap(),
        ),
        phase: MutationPhase::RegistryCommitted,
        worker: None,
        epoch: None,
        command: None,
        recovery_ref: None,
        provider_observation_digest: None,
        terminal_outcome: None,
        plan: Some(plan.clone()),
    };
    assert_eq!(
        match_record(&record, subject, auth, &plan),
        Err(DeliveryError::Unavailable)
    );
    record.phase = MutationPhase::Completed;
    assert_eq!(
        match_record(&record, subject, auth, &plan),
        Err(DeliveryError::Unavailable)
    );
    record.terminal_outcome = Some(MutationTerminalOutcome::Committed);
    assert!(match_record(&record, subject, auth, &plan).is_ok());
    assert_eq!(
        match_record(
            &record,
            subject,
            AuthenticatorId::from_bytes([7; 16]).unwrap(),
            &plan
        ),
        Err(DeliveryError::Unavailable)
    );
}
