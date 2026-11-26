use super::{Message, Reply};

/// Collects a frame of response and encodes.
pub fn encode(reply: &Reply, request_id: &[u8]) -> Vec<u8> {
    reply.frame(request_id).encode_to_vec()
}
