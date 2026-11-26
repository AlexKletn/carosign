/// The unsub.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubjectRef {
    /// A source of trust.
    #[prost(bytes = "vec", tag = "1")]
    pub authority_id: Vec<u8>,
    /// The unsub.
    #[prost(bytes = "vec", tag = "2")]
    pub subject_id: Vec<u8>,
}
