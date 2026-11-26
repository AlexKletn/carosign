use carosign_core::{enrollment::EnrollmentRef, ids::MutationId};

/// Securing the registry.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommitEnrollment {
    pub mutation: MutationId,
    /// A record that needs to be fixed (or deleted) `remove`).
    pub enrollment: EnrollmentRef,
    /// Whether it is registration (otherwise deletion).
    pub enroll: bool,
}
