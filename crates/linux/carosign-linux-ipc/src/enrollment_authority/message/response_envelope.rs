use prost::Message;

use super::response;

#[derive(Clone, PartialEq, Message)]
pub struct Response {
    #[prost(uint32, tag = "1")]
    pub(in crate::enrollment_authority) version: u32,
    #[prost(uint64, tag = "2")]
    pub(in crate::enrollment_authority) sequence: u64,
    #[prost(oneof = "response::Result", tags = "10, 11, 12, 13, 14")]
    pub result: Option<response::Result>,
}
