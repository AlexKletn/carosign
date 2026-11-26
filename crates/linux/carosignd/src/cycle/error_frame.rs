use super::*;

/// A frame of error for the client.
pub fn error_frame(request_id: &[u8], code: common::ErrorCode, message: &str) -> Vec<u8> {
    codec::error_frame(request_id, code, message).encode_to_vec()
}
