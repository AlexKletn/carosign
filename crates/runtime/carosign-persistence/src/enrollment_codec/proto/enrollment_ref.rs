/// Reference to registration.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EnrollmentRef {
    /// Identifier.
    #[prost(bytes = "vec", tag = "1")]
    pub enrollment_id: Vec<u8>,
    /// Authenticator.
    #[prost(bytes = "vec", tag = "2")]
    pub authenticator_id: Vec<u8>,
    /// Generation.
    #[prost(uint64, tag = "3")]
    pub generation: u64,
}
