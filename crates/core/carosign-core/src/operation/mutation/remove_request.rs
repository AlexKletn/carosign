use crate::enrollment::EnrollmentRef;
use crate::identity::SubjectRef;

/// Please delete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveRequest {
    /// Whose (to check the scope of authority)
    pub subject: SubjectRef,
    /// What exactly, with a fixed generation.
    pub enrollment: EnrollmentRef,
}
