use super::{BINDINGS_MAX, BindingKind, EnrollmentBinding, SetError};
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::ids::{EnrollmentId, ResourceId};
use core::fmt;

/// Set of authorized registrations.
#[derive(Clone, PartialEq, Eq)]
pub struct PermittedSet {
    bindings: Vec<EnrollmentBinding>,
}

impl PermittedSet {
    /// The checksum of the full set, including opaque ligaments.
    pub fn scoped_digest(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"carosign-permitted-set-v1\0");
        hash.update((self.bindings.len() as u64).to_be_bytes());
        for binding in &self.bindings {
            hash.update(binding.enrollment.id.as_bytes());
            hash.update(binding.enrollment.authenticator.as_bytes());
            hash.update(binding.enrollment.generation.value().to_be_bytes());
            hash.update([match binding.kind {
                BindingKind::ProviderOpaque => 1,
                BindingKind::ScopedVault => 2,
            }]);
            hash.update((binding.provider_ref.len() as u64).to_be_bytes());
            hash.update(&binding.provider_ref);
            match &binding.format_id {
                Some(format) => {
                    hash.update([1]);
                    hash.update((format.len() as u64).to_be_bytes());
                    hash.update(format.as_bytes());
                }
                None => hash.update([0]),
            }
            hash.update(binding.format_version.to_be_bytes());
        }
        hash.finalize().into()
    }

    /// He's making a set.
    ///
    /// ⚠️ **Empty set rejected** is the prohibition on "scan all" from document 29.
    pub fn new(bindings: Vec<EnrollmentBinding>) -> Result<Self, SetError> {
        if bindings.is_empty() {
            return Err(SetError::Empty);
        }
        if bindings.len() > BINDINGS_MAX {
            return Err(SetError::TooMany {
                actual: bindings.len(),
            });
        }
        for (index, binding) in bindings.iter().enumerate() {
            binding.validate()?;
            if bindings[..index]
                .iter()
                .any(|other| other.enrollment.id == binding.enrollment.id)
            {
                return Err(SetError::Duplicate);
            }
        }
        Ok(Self { bindings })
    }

    /// What's in the kit.
    pub fn bindings(&self) -> &[EnrollmentBinding] {
        &self.bindings
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Empty does not happen - this is a type guarantee, not a call check.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Is the registration included in the set ** with this generation **?
    ///
    /// ⚠️ Check the response of the provider: `matched_enrollment` It must be from the set,
    /// otherwise the result is discarded (Document 29, D-03).
    pub fn contains(&self, enrollment: EnrollmentRef) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.enrollment == enrollment)
    }

    /// The keys of the storage that must be issued (for `ScopedVault`).
    pub fn vault_resource(&self) -> Option<ResourceId> {
        None
    }

    /// List of registrations - for the journal and for the answer.
    pub fn enrollments(&self) -> Vec<EnrollmentId> {
        self.bindings
            .iter()
            .map(|binding| binding.enrollment.id)
            .collect()
    }
}

impl fmt::Debug for PermittedSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PermittedSet({} связок)", self.bindings.len())
    }
}

#[path = "permitted_set.tests.rs"]
#[cfg(test)]
mod tests;
