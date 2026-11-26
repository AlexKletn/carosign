use carosign_linux_ipc::client::{
    transport::Transport,
    verification_events::{Observation, observe_verification_with_acceptance},
};
use carosign_protocol::carosign::{
    client::v1::{BeginVerification, GetHealth, client_frame::Payload},
    common::v1::{SubjectSelector, VerificationPurpose, subject_selector::Selector},
};

#[test]
#[ignore = "requires the isolated installed stack with the persistent mock provider"]
fn repeated_verification_on_one_connection_exceeds_the_live_registry_limit() {
    let socket = std::env::var("CAROSIGN_LONG_RUN_SOCKET").unwrap();
    let transport = Transport::connect_root(&socket).unwrap();
    for sequence in 1..=300_u16 {
        let mut idempotency_key = vec![0; 16];
        idempotency_key[14..].copy_from_slice(&sequence.to_be_bytes());
        let observed = observe_verification_with_acceptance(
            &transport,
            BeginVerification {
                enrollment: None,
                report_matched_enrollment: false,
                idempotency_key,
                subject: Some(SubjectSelector {
                    selector: Some(Selector::AccountHint("root".into())),
                }),
                authenticator_id: vec![3; 16],
                purpose_hint: VerificationPurpose::PurposeSessionLogin as i32,
                authority_request_ref: Vec::new(),
            },
            |_| {},
        );
        assert!(
            matches!(observed, Ok(Observation::FactorAccepted)),
            "verification {sequence}: {observed:?}"
        );
    }
    let health = transport.request(Payload::GetHealth(GetHealth {})).unwrap();
    assert!(matches!(health.payload, Some(Payload::Health(_))));
}
