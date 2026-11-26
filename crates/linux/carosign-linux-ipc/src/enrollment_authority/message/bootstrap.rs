use prost::Message;

use super::GrantBinding;

/// Initial linking of a closed endpoint to the transaction area.Payload is owned by the
/// recipient, is not considered an authority and should not contain secrets.
#[derive(Clone, PartialEq, Message)]
pub(in crate::enrollment_authority) struct Bootstrap {
    #[prost(uint32, tag = "1")]
    pub(in crate::enrollment_authority) version: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub(in crate::enrollment_authority) scope: Vec<u8>,
    #[prost(message, optional, tag = "3")]
    pub(in crate::enrollment_authority) grant: Option<GrantBinding>,
    #[prost(bytes = "vec", tag = "4")]
    pub(in crate::enrollment_authority) payload: Vec<u8>,
}
