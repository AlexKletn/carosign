use carosign_protocol::carosign::broker_relay::v1::{Frame, Hello, frame::Body};

pub(super) fn hello_frame() -> Frame {
    Frame {
        major: 1,
        minor: 0,
        seq: 1,
        worker_instance: vec![1; 16],
        device_epoch: 1,
        body: Some(Body::Hello(Hello {
            broker_instance: vec![2; 16],
            boot_id: vec![3; 16],
        })),
    }
}
