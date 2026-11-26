use super::{BindingError, BindingKind, ProviderReference};
use carosign_core::enrollment::EnrollmentRef;
use core::fmt;

/// One authorized registration.
///
/// ⚠️ No template bytes: only link and metadata; no template bytes go through the control
/// channel (Document 29, D-07).
#[derive(Clone, PartialEq, Eq)]
pub struct EnrollmentBinding {
    /// What kind of registration and with what generation?
    pub enrollment: EnrollmentRef,
    /// Where's the template?
    pub kind: BindingKind,
    /// Reference of the provider (for `ProviderOpaque`, up to 4,096 bytes.
    pub provider_ref: ProviderReference,
    /// The format of the template, if the provider announced it.
    pub format_id: Option<String>,
    pub format_version: u32,
}

impl fmt::Debug for EnrollmentBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnrollmentBinding")
            .field("kind", &self.kind)
            .field("provider_ref_len", &self.provider_ref.len())
            .finish_non_exhaustive()
    }
}

impl EnrollmentBinding {
    /// Checking the shape of the ligament.
    ///
    /// ⚠️ An empty opaque reference is only permissible where the provider restores the
    /// ligament from its permanent state (document 29, D-03).
    pub fn validate(&self) -> Result<(), BindingError> {
        if self.provider_ref.len() > 4096 {
            return Err(BindingError::ReferenceTooLarge {
                actual: self.provider_ref.len(),
            });
        }
        if matches!(&self.format_id, Some(format) if format.len() > 128) {
            return Err(BindingError::FormatTooLong);
        }
        if self.kind == BindingKind::ScopedVault && !self.provider_ref.is_empty() {
            // ⚠️ For storage, the link is the registration itself, not other people’s bytes:
            // you can not mix.
            return Err(BindingError::KindMismatch);
        }
        Ok(())
    }

    /// Does the registration match (with the generation)?
    pub fn is(&self, enrollment: EnrollmentRef) -> bool {
        self.enrollment == enrollment
    }
}
