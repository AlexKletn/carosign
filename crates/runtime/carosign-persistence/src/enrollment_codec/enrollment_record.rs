use super::{EnrollmentRef, EnrollmentStatus, OpaqueProviderRef, SubjectRef, fmt};

/// Domain registry record.
#[derive(Clone, PartialEq, Eq)]
pub struct EnrollmentRecord {
    /// Who it belongs to.
    pub subject: SubjectRef,
    /// What kind of registration and with what generation?
    pub enrollment: EnrollmentRef,
    /// A name for a client.
    pub label: String,
    pub display_label: Option<String>,
    pub status: EnrollmentStatus,
    /// Provider's link (not template).
    pub provider_ref: OpaqueProviderRef,
    pub format_id: String,
    pub format_version: u32,
    /// Type of storage (document 34): mandatory, zero is not allowed.
    pub binding_kind: crate::mutation_plan::StoredBindingKind,
}

impl EnrollmentRecord {
    pub fn display_label(&self) -> &str {
        self.display_label.as_deref().unwrap_or(&self.label)
    }
}

impl fmt::Debug for EnrollmentRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnrollmentRecord")
            .field("status", &self.status)
            .field("generation", &self.enrollment.generation)
            .finish_non_exhaustive()
    }
}
