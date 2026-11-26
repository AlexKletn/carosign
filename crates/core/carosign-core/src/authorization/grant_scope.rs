use crate::enrollment::EnrollmentRef;
use crate::identity::SubjectRef;
use crate::ids::AuthenticatorId;
use crate::ids::OperationId;
use crate::modality::OperationKind;
use crate::operation::context::VerificationPurpose;
use core::fmt;

/// What's allowed.
#[derive(Clone, PartialEq, Eq)]
pub enum GrantScope {
    /// Testing the subject on the apparatus for the named purpose.
    Verify {
        subject: SubjectRef,
        authenticator: AuthenticatorId,
        /// Purpose of inspection.
        purpose: VerificationPurpose,
    },
    /// Registration (with optional replacement by generation).
    Enroll {
        subject: SubjectRef,
        authenticator: AuthenticatorId,
        /// Precise replacement registration, if any.
        replace: Option<EnrollmentRef>,
    },
    /// Deletion of specific registration.
    Remove {
        subject: SubjectRef,
        /// Removable registration with a generation.
        enrollment: EnrollmentRef,
    },
    /// Surveillance of the operation.
    Observe(OperationId),
    /// Cancel the operation.
    Cancel(OperationId),
    /// Provider management is **only a system policy**.
    ManageProvider,
}

impl fmt::Debug for GrantScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verify { .. } => f.write_str("Verify(..)"),
            Self::Enroll { .. } => f.write_str("Enroll(..)"),
            Self::Remove { .. } => f.write_str("Remove(..)"),
            Self::Observe(_) => f.write_str("Observe(..)"),
            Self::Cancel(_) => f.write_str("Cancel(..)"),
            Self::ManageProvider => f.write_str("ManageProvider"),
        }
    }
}

impl GrantScope {
    /// The subject of the area, if the area is tied to it.
    pub fn subject(&self) -> Option<SubjectRef> {
        match self {
            Self::Verify { subject, .. }
            | Self::Enroll { subject, .. }
            | Self::Remove { subject, .. } => Some(*subject),
            Self::Observe(_) | Self::Cancel(_) | Self::ManageProvider => None,
        }
    }

    /// Transaction if the permission is addressed to a specific operation.
    pub fn operation(&self) -> Option<OperationId> {
        match self {
            Self::Observe(operation) | Self::Cancel(operation) => Some(*operation),
            _ => None,
        }
    }

    /// Whether the area is subject to change (journalized before sending).
    pub fn is_mutating(&self) -> bool {
        matches!(self, Self::Enroll { .. } | Self::Remove { .. })
    }

    /// The type of operation, if the area is about it.
    pub fn operation_kind(&self) -> Option<OperationKind> {
        match self {
            Self::Verify { .. } => Some(OperationKind::Verify),
            Self::Enroll { .. } => Some(OperationKind::Enroll),
            Self::Remove { .. } => Some(OperationKind::Remove),
            _ => None,
        }
    }
}
