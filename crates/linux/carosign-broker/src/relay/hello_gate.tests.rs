use super::super::test_fixture::hello_frame;
use super::*;
use carosign_protocol::carosign::broker_relay::v1::Hello;

#[test]
fn initial_hello_refuses_extra_fields_fds_and_replay() {
    let mut gate = HelloGate::new([1; 16], 1).unwrap();
    assert_eq!(
        gate.accept(&hello_frame().encode_to_vec(), 0),
        Ok(([2; 16], [3; 16]))
    );
    assert_eq!(
        gate.accept(&hello_frame().encode_to_vec(), 0),
        Err(Refusal::Closed)
    );
    let mut duplicate = hello_frame().encode_to_vec();
    duplicate.extend_from_slice(&[0x08, 0x01]);
    assert_eq!(
        HelloGate::new([1; 16], 1).unwrap().accept(&duplicate, 0),
        Err(Refusal::Invalid)
    );
    let mut unknown = hello_frame().encode_to_vec();
    unknown.extend_from_slice(&[0xa0, 0x01, 0x01]);
    assert_eq!(
        HelloGate::new([1; 16], 1).unwrap().accept(&unknown, 0),
        Err(Refusal::Invalid)
    );
    assert_eq!(
        HelloGate::new([1; 16], 1)
            .unwrap()
            .accept(&hello_frame().encode_to_vec(), 1),
        Err(Refusal::Invalid)
    );
}

#[test]
fn request_id_is_deterministic_unique_and_nonzero() {
    assert_ne!(request_id([9; 16], 1), request_id([9; 16], 2));
    assert_eq!(request_id([9; 16], 0), Err(Refusal::Invalid));
    let mut instance = [0; 16];
    instance[15] = 1;
    assert_eq!(request_id(instance, 1), Err(Refusal::Invalid));
}

#[test]
fn hello_rejects_bad_context_and_invalid_nested_fields() {
    let mut bad = hello_frame();
    bad.seq = 2;
    assert_eq!(
        HelloGate::new([1; 16], 1)
            .unwrap()
            .accept(&bad.encode_to_vec(), 0),
        Err(Refusal::Invalid)
    );
    bad = hello_frame();
    bad.device_epoch = 2;
    assert_eq!(
        HelloGate::new([1; 16], 1)
            .unwrap()
            .accept(&bad.encode_to_vec(), 0),
        Err(Refusal::Invalid)
    );
    bad = hello_frame();
    bad.worker_instance = vec![4; 16];
    assert_eq!(
        HelloGate::new([1; 16], 1)
            .unwrap()
            .accept(&bad.encode_to_vec(), 0),
        Err(Refusal::Invalid)
    );
    let mut nested = Hello {
        broker_instance: vec![2; 16],
        boot_id: vec![3; 16],
    }
    .encode_to_vec();
    nested.extend_from_slice(&[0x0a, 0x01, 0x02]);
    let mut raw = vec![0x08, 0x01, 0x18, 0x01, 0x22, 0x10];
    raw.extend_from_slice(&[1; 16]);
    raw.extend_from_slice(&[0x28, 0x01, 0x52, nested.len() as u8]);
    raw.extend_from_slice(&nested);
    assert_eq!(
        HelloGate::new([1; 16], 1).unwrap().accept(&raw, 0),
        Err(Refusal::Invalid)
    );
    assert_eq!(
        HelloGate::new([1; 16], 1)
            .unwrap()
            .accept(&vec![0; carosign_protocol::limits::FRAME_MAX + 1], 0),
        Err(Refusal::Invalid)
    );
}
