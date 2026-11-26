use super::*;

#[test]
fn refusal_message_is_bounded_in_utf8_bytes() {
    let message = "а".repeat(200);
    let ack = refusal(
        carosign_protocol::carosign::common::v1::ErrorCode::ErrorBusy,
        &message,
    );
    let text = &ack.rejection.unwrap().safe_message;
    assert_eq!(text.len(), carosign_protocol::limits::SAFE_MESSAGE_MAX);
    assert!(message.starts_with(text));

    let boundary = format!("{}😀", "a".repeat(255));
    let ack = refusal(
        carosign_protocol::carosign::common::v1::ErrorCode::ErrorBusy,
        &boundary,
    );
    assert_eq!(ack.rejection.unwrap().safe_message, "a".repeat(255));
}
