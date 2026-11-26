use prost::Message;

/// `reference` A link to the challenge at Authorize and to the resolution at Revalidate. No
/// link is enough without an independent verification by the domain.
#[derive(Clone, PartialEq, Message)]
pub struct Authorization {
    #[prost(bytes = "vec", tag = "1")]
    pub reference: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub challenge_digest: Vec<u8>,
}
