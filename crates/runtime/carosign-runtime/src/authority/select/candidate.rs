use super::Custody;
use super::SelectionError;
use super::Status;
use crate::operation::bindings::BindingKind;
use crate::operation::bindings::EnrollmentBinding;
use crate::operation::bindings::PermittedSet;
use carosign_core::device::DeviceEpoch;
use carosign_core::enrollment::EnrollmentRef;
use carosign_core::enrollment::OpaqueProviderRef;
use carosign_core::identity::SubjectRef;
use carosign_core::ids::AuthenticatorId;
use core::fmt;

/// What you need to know about a registry record to decide if it is fit.
///
/// ⚠️ A separate type, not a storage record: a registry crate should not depend on runtime,
/// and selection needs exactly that.
#[derive(Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Who it belongs to.
    pub subject: SubjectRef,
    /// What kind of registration and with what generation?
    pub enrollment: EnrollmentRef,
    /// Status of the record (`ENROLLMENT_ACTIVE` and others.
    pub status: Status,
    /// Where's the template?
    pub custody: Custody,
    /// Non-transparent link provider.
    pub provider_ref: Vec<u8>,
    pub format_id: Option<String>,
    pub format_version: u32,
}

impl fmt::Debug for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Candidate")
            .field("status", &self.status)
            .field("custody", &self.custody)
            .finish_non_exhaustive()
    }
}

/// Selects the approved set.
///
/// ⚠️ The order of checks is given by the meaning:
///
/// 1. the record belongs to **this ** authenticator - otherwise the registry is corrupted; 2.
/// the record is **active** - otherwise it is not checked; 3. the record ** does not wait for
/// reconciliation** - otherwise it is turned off (document 10, "Recovery boot gate"); 4. the
/// generation is taken ** from the record **, not the "last": the replacement of the
/// generation would rest on the provider.
///
/// ⚠️ `epoch` It's transmitted for clarity of call, but **not* is compared to the record: the
/// era is a property of the device, not the registration. The obsolete era is cut off
/// earlier, when leasing.
pub fn select(
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    epoch: DeviceEpoch,
    candidates: &[Candidate],
) -> Result<PermittedSet, SelectionError> {
    let _ = epoch;
    let mut bindings: Vec<EnrollmentBinding> = Vec::new();

    for candidate in candidates {
        if candidate.enrollment.authenticator != authenticator {
            return Err(SelectionError::WrongAuthenticator);
        }
        if candidate.subject != subject {
            // ⚠️ The alien subject is not "missed": it is a sample corruption.
            return Err(SelectionError::WrongAuthenticator);
        }
        match candidate.status {
            // ⚠️ Waiting for reconciliation is a refusal to **all ** check: such a record can
            // not be checked, and "to pass it silently" means to give someone else's
            // fingerprint for your own.
            Status::PendingReconciliation => return Err(SelectionError::PendingReconciliation),
            // Unavailable just isn't included in the set.
            Status::Unavailable => continue,
            Status::Active => {}
        }
        bindings.push(EnrollmentBinding {
            enrollment: candidate.enrollment,
            kind: match candidate.custody {
                Custody::Provider => BindingKind::ProviderOpaque,
                Custody::PlatformVault => BindingKind::ScopedVault,
            },
            // Checking the set will reject the link ScopedVault; You can not correct the
            // damaged binding by silent removal of bytes.
            provider_ref: candidate.provider_ref.clone(),
            format_id: candidate.format_id.clone(),
            format_version: candidate.format_version,
        });
    }

    if bindings.is_empty() {
        return Err(SelectionError::NotEnrolled);
    }

    Ok(PermittedSet::new(bindings)?)
}

impl Candidate {
    /// From the registry record.
    ///
    /// ⚠️ The only way to “record storage → candidate”: the status is taken from the record,
    /// and the type of storage is from the explicit type of binding.
    pub fn from_record(record: &carosign_persistence::enrollment_codec::EnrollmentRecord) -> Self {
        use carosign_persistence::enrollment_codec::EnrollmentStatus as Stored;
        let status = match record.status {
            Stored::Active => Status::Active,
            Stored::PendingReconciliation => Status::PendingReconciliation,
            Stored::Unavailable => Status::Unavailable,
        };
        // ⚠️ The type of storage is taken ** from the record ** (document 34): an empty
        // reference does not mean a repository, and it is forbidden to guess by it.
        let custody = match record.binding_kind {
            carosign_persistence::mutation_plan::StoredBindingKind::ProviderOpaque => {
                Custody::Provider
            }
            carosign_persistence::mutation_plan::StoredBindingKind::ScopedVault => {
                Custody::PlatformVault
            }
        };
        Self {
            subject: record.subject,
            enrollment: record.enrollment,
            status,
            custody,
            provider_ref: record.provider_ref.as_bytes().to_vec(),
            format_id: if record.format_id.is_empty() {
                None
            } else {
                Some(record.format_id.clone())
            },
            format_version: record.format_version,
        }
    }

    /// Whether the matching tape is waiting.
    pub fn waits_reconciliation(&self) -> bool {
        self.status == Status::PendingReconciliation
    }

    /// An opaque link as a type of domain.
    pub fn opaque(&self) -> Option<OpaqueProviderRef> {
        OpaqueProviderRef::new(self.provider_ref.clone()).ok()
    }
}

#[path = "candidate.tests.rs"]
#[cfg(test)]
mod tests;
