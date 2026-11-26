/// What's inside the meaning.
#[derive(Clone, PartialEq, ::prost::Oneof)]
pub enum Value {
    /// Registry record.
    #[prost(message, tag = "11")]
    Enrollment(super::EnrollmentRecord),
}
