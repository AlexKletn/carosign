use super::*;
use carosign_provider_sdk::events::Event;

#[test]
fn challenge_is_not_accepted_without_trusted_session_validation() {
    let event = Event::EnrollmentChallenge {
        worker: vec![1; 16],
        device_epoch: 1,
        command: vec![2; 16],
        operation: vec![3; 16],
        mutation: vec![4; 16],
        challenge: vec![5],
        challenge_ref: vec![6; 16],
        validity_ms: 100,
    };
    assert!(to_domain(event).is_err());
}

#[test]
fn progress_is_not_silently_stripped_of_its_context() {
    let event = Event::Progress {
        capture_state: None,
        feedback: None,
        worker: vec![2; 16],
        device_epoch: 1,
        command: vec![3; 16],
        operation: vec![1; 16],
        sequence: 1,
        kind: provider::ProgressKind::ProgressCapturing,
        percent: 45,
    };
    assert!(to_domain(event).is_err());
}

#[test]
fn recovery_reference_is_not_silently_dropped() {
    let event = Event::Mutation {
        worker: vec![4; 16],
        device_epoch: 1,
        command: vec![5; 16],
        operation: vec![1; 16],
        mutation: vec![2; 16],
        status: provider::MutationStatus::MutationMayHaveOccurred,
        recovery_ref: vec![3],
        provider_ref: Vec::new(),
        enrollment: None,
    };
    assert!(to_domain(event).is_err());
}

#[test]
fn provider_reference_is_not_silently_dropped() {
    let event = Event::Mutation {
        worker: vec![4; 16],
        device_epoch: 1,
        command: vec![5; 16],
        operation: vec![1; 16],
        mutation: vec![2; 16],
        status: provider::MutationStatus::MutationProviderCompleted,
        recovery_ref: Vec::new(),
        provider_ref: vec![3],
        enrollment: None,
    };
    assert!(to_domain(event).is_err());
}

#[test]
fn provider_query_alone_cannot_prove_reconciliation() {
    for state in [
        provider::QueryState::QueryConfirmedPresent,
        provider::QueryState::QueryConfirmedAbsent,
        provider::QueryState::QueryUnknown,
        provider::QueryState::QueryUnsupported,
    ] {
        assert!(
            to_domain(Event::QueryState {
                mutation: vec![1; 16],
                state,
            })
            .is_err()
        );
    }
    assert!(
        to_domain(Event::Fault {
            error: common::Error::default()
        })
        .is_err()
    );
}

#[test]
fn session_events_are_not_misrepresented_as_operation_progress() {
    let events = [
        Event::Accepted {
            command: vec![1; 16],
            rejection: None,
        },
        Event::CancelReceipt {
            status: provider::CancelStatus::default(),
        },
        Event::Quiesced { claimed: true },
        Event::Described {
            capabilities: common::CapabilityDescriptor::default(),
        },
    ];
    for event in events {
        assert!(to_domain(event).is_err());
    }
}

#[test]
fn worker_bytes_do_not_become_a_trusted_evidence_id() {
    use carosign_provider_sdk::events::ProviderEvidence;
    for evidence in [
        None,
        Some(ProviderEvidence::Reference(vec![7; 16])),
        Some(ProviderEvidence::OpaqueToken(vec![8; 32])),
    ] {
        let event = Event::Verification {
            worker: vec![2; 16],
            device_epoch: 1,
            command: vec![3; 16],
            operation: vec![1; 16],
            status: provider::VerificationStatus::VerifyMatched,
            evidence,
            format_id: String::new(),
            matched_enrollment: None,
            error: None,
        };
        assert!(to_domain(event).is_err());
    }
    for status in [
        provider::VerificationStatus::VerifyNotMatched,
        provider::VerificationStatus::VerifyNoUsableSample,
        provider::VerificationStatus::VerifyCancelled,
    ] {
        let event = Event::Verification {
            worker: vec![2; 16],
            device_epoch: 1,
            command: vec![3; 16],
            operation: vec![1; 16],
            status,
            evidence: None,
            format_id: String::new(),
            matched_enrollment: None,
            error: None,
        };
        assert!(to_domain(event).is_err());
    }
}
