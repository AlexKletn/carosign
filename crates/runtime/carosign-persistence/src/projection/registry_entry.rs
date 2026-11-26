use super::{EnrollmentRecord, EnrollmentRef, EnrollmentStatus, StoredBindingKind, fmt};

/// What does the collector need to record the registry?
///
/// ⚠️ A separate type, not the record itself: the collector needs only those fields by which
/// he compares - and not a single extra.
#[derive(Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub enrollment: EnrollmentRef,
    /// Subject (to check with the plan).
    pub subject: carosign_core::identity::SubjectRef,
    /// Record status.
    pub status: EnrollmentStatus,
    /// Storage type.
    pub kind: StoredBindingKind,
    /// Bytes of an opaque link (for digest).
    pub provider_ref: Vec<u8>,
    pub format_id: String,
    pub format_version: u32,
}

impl fmt::Debug for RegistryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistryEntry")
            .field("status", &self.status)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl RegistryEntry {
    /// From the registry record.
    pub fn from_record(record: &EnrollmentRecord) -> Self {
        Self {
            enrollment: record.enrollment,
            subject: record.subject,
            status: record.status,
            kind: record.binding_kind,
            provider_ref: record.provider_ref.as_bytes().to_vec(),
            format_id: record.format_id.clone(),
            format_version: record.format_version,
        }
    }
}
