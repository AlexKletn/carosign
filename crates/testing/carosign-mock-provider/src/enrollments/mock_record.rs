use carosign_protocol::carosign::common::v1 as common;

#[derive(Clone, PartialEq, prost::Message)]
pub(super) struct MockRecord {
    #[prost(message, optional, tag = "1")]
    pub(super) subject: Option<common::SubjectRef>,
    #[prost(message, optional, tag = "2")]
    pub(super) enrollment: Option<common::EnrollmentRef>,
    #[prost(bytes = "vec", tag = "3")]
    pub(super) opaque: Vec<u8>,
}
