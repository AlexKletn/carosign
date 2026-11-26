use carosign_protocol::carosign::client::v1 as client;
use carosign_protocol::carosign::client::v1::client_frame::Payload;

pub fn hello_ack(
    request_id: &[u8],
    major: u32,
    selected_minor: u32,
    connection_id: [u8; 16],
    capture_feedback_v1: bool,
) -> client::ClientFrame {
    client::ClientFrame {
        major,
        minor: selected_minor,
        request_id: request_id.to_vec(),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::HelloAck(client::HelloAck {
            verification_enrollment_selector: true,
            capture_feedback_v1,
            major,
            selected_minor,
            connection_id: connection_id.to_vec(),
        })),
    }
}
