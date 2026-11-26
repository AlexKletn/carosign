use super::BindingKind;
use crate::enrollment::EnrollmentRef;
use crate::identity::SubjectRef;

/// Original bundle without template bytes or an open link from the provider.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BindingSnapshot {
    /// Accurate registration.
    pub enrollment: EnrollmentRef,
    /// Registrant.
    pub subject: SubjectRef,
    /// Storage type.
    pub kind: BindingKind,
    /// Digest of opaque reference.
    pub provider_ref_digest: [u8; 32],
    pub format_id: String,
    pub format_version: u32,
}
