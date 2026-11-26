use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct Approved {
    #[prost(bytes = "vec", tag = "1")]
    pub reference: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub challenge_digest: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub deadline_ns: u64,
    /// Non-transparent, non-secret linkage of the owner, only for Authorize.
    #[prost(bytes = "vec", tag = "4")]
    pub owner_binding: Vec<u8>,
}
