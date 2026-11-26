use carosign_protocol::carosign::client::v1 as client;
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::carosign::common::v1 as common;

pub fn error_frame(
    request_id: &[u8],
    code: common::ErrorCode,
    message: &str,
) -> client::ClientFrame {
    client::ClientFrame {
        major: carosign_protocol::limits::MAJOR,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        request_id: request_id.to_vec(),
        operation_id: Vec::new(),
        event_sequence: 0,
        payload: Some(Payload::ProtocolError(client::ProtocolError {
            error: Some(common::Error {
                code: code as i32,
                safe_message: message
                    .char_indices()
                    .take_while(|(offset, ch)| {
                        offset + ch.len_utf8() <= carosign_protocol::limits::SAFE_MESSAGE_MAX
                    })
                    .map(|(_, ch)| ch)
                    .collect(),
            }),
        })),
    }
}

#[cfg(test)]
#[path = "error_frame.tests.rs"]
mod tests;
