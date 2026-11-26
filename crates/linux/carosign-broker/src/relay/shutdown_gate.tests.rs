use super::*;

#[test]
fn shutdown_gate_checks_wire_context_sequence_and_replay() {
    use carosign_protocol::carosign::broker_relay::v1::Command;
    use carosign_protocol::carosign::provider::v1::{ProviderFrame, Shutdown, provider_frame::Payload};
    let worker = [1; 16];
    let instance = [3; 16];
    let command = Frame {
        major: 1,
        minor: 0,
        seq: 2,
        worker_instance: worker.to_vec(),
        device_epoch: 7,
        body: Some(Body::Command(Command {
            request_id: request_id(instance, 2).unwrap().to_vec(),
            frame: Some(ProviderFrame {
                major: 1,
                minor: carosign_protocol::limits::MINOR_BASELINE,
                worker_instance_id: worker.to_vec(),
                frame_seq: 0,
                command_id: vec![2; 16],
                operation_id: vec![],
                device_epoch: 7,
                uid: None,
                payload: Some(Payload::Shutdown(Shutdown {})),
            }),
            mutation_permit: None,
        })),
    };
    let bytes = command.encode_to_vec();
    let mut gate = ShutdownGate::new(worker, 7, instance).unwrap();
    assert!(gate.accept(&bytes).is_ok());
    assert!(gate.accept(&bytes).is_err());
    assert_eq!(gate.accept(&bytes), Err(Refusal::Closed));
    let mut duplicated = bytes.clone();
    // Repeating the singular field cannot be hidden by re-decoding.
    duplicated.extend_from_slice(&[0x08, 0x01]);
    let mut gate = ShutdownGate::new(worker, 7, instance).unwrap();
    assert!(gate.accept(&duplicated).is_err());
    let mut gate = ShutdownGate::new(worker, 8, instance).unwrap();
    assert!(gate.accept(&bytes).is_err());
}
