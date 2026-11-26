use super::*;

#[test]
fn error_text_is_limited_by_utf8_bytes() {
    let frame = error_frame(&[1; 16], common::ErrorCode::ErrorInternal, &"а".repeat(200));
    let Some(Payload::ProtocolError(error)) = frame.payload else {
        panic!("ожидалась ошибка")
    };
    assert_eq!(
        error.error.unwrap().safe_message.len(),
        carosign_protocol::limits::SAFE_MESSAGE_MAX
    );
    let frame = error_frame(
        &[1; 16],
        common::ErrorCode::ErrorInternal,
        &format!("{}😀", "a".repeat(255)),
    );
    let Some(Payload::ProtocolError(error)) = frame.payload else {
        panic!("ожидалась ошибка")
    };
    assert_eq!(error.error.unwrap().safe_message, "a".repeat(255));
}
