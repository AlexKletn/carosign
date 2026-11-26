use super::{EnrollmentRef, SubjectRef};

/// Registry record.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EnrollmentRecord {
    /// Registry.
    #[prost(message, optional, tag = "1")]
    pub enrollment: Option<EnrollmentRef>,
    /// The unsub.
    #[prost(message, optional, tag = "2")]
    pub subject: Option<SubjectRef>,
    /// Mark.
    #[prost(string, tag = "3")]
    pub label: String,
    /// Status.
    #[prost(int32, tag = "4")]
    pub status: i32,
    /// ISP link.
    #[prost(bytes = "vec", tag = "5")]
    pub provider_opaque_ref: Vec<u8>,
    /// Pattern format.
    #[prost(string, tag = "6")]
    pub template_format_id: String,
    /// The format version.
    #[prost(uint32, tag = "7")]
    pub template_format_version: u32,
    /// Type of storage (mandatory with v3).
    #[prost(int32, tag = "8")]
    pub binding_kind: i32,
    #[prost(string, optional, tag = "9")]
    pub display_label: Option<String>,
}
