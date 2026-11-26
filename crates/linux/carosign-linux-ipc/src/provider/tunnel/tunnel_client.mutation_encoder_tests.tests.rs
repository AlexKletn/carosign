use super::super::HelloError;

use super::*;

#[test]
fn ordinary_command_cannot_send_a_mutation() {
    for payload in [
        Payload::StartEnrollment(Default::default()),
        Payload::StartRemoval(Default::default()),
    ] {
        let mut encoder = TunnelCommandEncoder::after_hello([1; 16], 7, [3; 16]).unwrap();
        let frame = ProviderFrame {
            major: 1,
            minor: 3,
            worker_instance_id: vec![1; 16],
            device_epoch: 7,
            payload: Some(payload),
            ..Default::default()
        };
        assert!(matches!(encoder.command(frame), Err(HelloError::Invalid)));
    }
}

#[test]
fn mutation_outcome_has_a_strict_wire_shape() {
    let event = Frame {
        major: 1,
        minor: 0,
        seq: 1,
        worker_instance: vec![1; 16],
        device_epoch: 7,
        body: Some(Body::Event(
            carosign_protocol::carosign::broker_relay::v1::Event {
                frame: Some(ProviderFrame {
                    major: 1,
                    minor: 3,
                    worker_instance_id: vec![1; 16],
                    frame_seq: 1,
                    command_id: vec![9; 16],
                    operation_id: vec![8; 16],
                    device_epoch: 7,
                    uid: None,
                    payload: Some(Payload::MutationOutcome(
                        carosign_protocol::carosign::provider::v1::MutationOutcome {
                            mutation_id: vec![13; 16],
                            status: 1,
                            enrollment: Some(carosign_protocol::carosign::common::v1::EnrollmentRef {
                                enrollment_id: vec![12; 16],
                                authenticator_id: vec![5; 16],
                                generation: 1,
                            }),
                            provider_opaque_ref: vec![1],
                            ..Default::default()
                        },
                    )),
                }),
            },
        )),
    };
    assert!(strict_event(&event.encode_to_vec()).is_ok());
}
