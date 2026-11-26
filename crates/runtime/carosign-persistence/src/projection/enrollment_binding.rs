use super::{EnrollmentRef, StoredBindingKind, fmt};

/// Checked replacement ligament.
#[derive(Clone, PartialEq, Eq)]
pub struct EnrollmentBinding {
    pub enrollment: EnrollmentRef,
    /// Storage type.
    pub kind: StoredBindingKind,
    /// ISP link.
    pub provider_ref: Vec<u8>,
    pub format_id: Option<String>,
    pub format_version: u32,
}

impl fmt::Debug for EnrollmentBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnrollmentBinding")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
