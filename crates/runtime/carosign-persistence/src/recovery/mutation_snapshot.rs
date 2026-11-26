use super::MutationRecord;

/// Consistent reading of incomplete mutation and indexed region.
///
/// Registry records are metadata, not an independent device certificate.
/// Checking the origin and physical condition remains with the restorer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MutationSnapshot {
    /// Verified v3 log entry.
    pub mutation: MutationRecord,
    /// Linked queue recordings in the same shot.
    pub outbox: Vec<crate::outbox::EffectOutboxRecord>,
    /// Registry records on the index of the blocked area.
    pub enrollments: Vec<crate::enrollment_codec::EnrollmentRecord>,
}
