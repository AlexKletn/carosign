use core::fmt;

use carosign_core::enrollment::EnrollmentRef;
use carosign_core::identity::SubjectRef;

use super::StoredBindingKind;

/// A snapshot of one registration **to* change.
#[derive(Clone, PartialEq, Eq)]
pub struct BindingSnapshot {
    pub enrollment: EnrollmentRef,
    pub subject: SubjectRef,
    /// Type of storage (document 34).
    pub kind: StoredBindingKind,
    /// SHA-256 is an opaque reference provider.
    pub provider_ref_digest: [u8; 32],
    pub format_id: String,
    pub format_version: u32,
}

impl fmt::Debug for BindingSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingSnapshot")
            .field("generation", &self.enrollment.generation)
            .finish_non_exhaustive()
    }
}
