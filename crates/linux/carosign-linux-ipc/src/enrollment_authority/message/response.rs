use prost::Oneof;

#[derive(Clone, PartialEq, Oneof)]
pub enum Result {
    #[prost(message, tag = "10")]
    Accepted(super::Empty),
    #[prost(message, tag = "11")]
    Validated(super::Approved),
    #[prost(message, tag = "12")]
    Authorized(super::Approved),
    #[prost(message, tag = "13")]
    Refused(super::Empty),
    #[prost(message, tag = "14")]
    Challenge(super::Validate),
}
