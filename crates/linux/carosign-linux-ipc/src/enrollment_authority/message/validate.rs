use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct Validate {
    #[prost(bytes = "vec", tag = "1")]
    pub challenge: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub challenge_ref: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub validity_ms: u64,
}
