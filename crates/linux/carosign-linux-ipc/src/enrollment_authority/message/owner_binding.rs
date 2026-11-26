use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct OwnerBinding {
    #[prost(bytes = "vec", tag = "1")]
    pub payload: Vec<u8>,
}
