use prost::Message;

use super::{GrantBinding, request};

/// Only links and surveillance; neither password nor token in the request are allowed.
#[derive(Clone, PartialEq, Message)]
pub struct Request {
    #[prost(uint32, tag = "1")]
    pub(in crate::enrollment_authority) version: u32,
    #[prost(uint64, tag = "2")]
    pub(in crate::enrollment_authority) sequence: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub(in crate::enrollment_authority) scope: Vec<u8>,
    #[prost(message, optional, tag = "4")]
    pub grant: Option<GrantBinding>,
    #[prost(oneof = "request::Action", tags = "10, 11, 12, 13, 14, 15, 16, 17, 18")]
    pub action: Option<request::Action>,
}
