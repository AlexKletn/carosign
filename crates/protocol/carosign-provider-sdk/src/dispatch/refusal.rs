use carosign_protocol::carosign::provider::v1 as provider;

/// The rejection that the worker returns to the platform.
pub fn refusal(
    code: carosign_protocol::carosign::common::v1::ErrorCode,
    message: &str,
) -> provider::CommandAck {
    provider::CommandAck {
        accepted: false,
        rejection: Some(carosign_protocol::carosign::common::v1::Error {
            code: code as i32,
            safe_message: message
                .char_indices()
                .take_while(|(offset, ch)| {
                    offset + ch.len_utf8() <= carosign_protocol::limits::SAFE_MESSAGE_MAX
                })
                .map(|(_, ch)| ch)
                .collect(),
        }),
    }
}

#[cfg(test)]
#[path = "refusal.tests.rs"]
mod tests;
