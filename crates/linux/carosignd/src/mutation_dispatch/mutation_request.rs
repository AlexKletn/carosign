use super::{EnrollmentLabel, EnrollmentRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MutationRequest {
    Enroll {
        label: EnrollmentLabel,
        replace: Option<EnrollmentRef>,
    },
    Remove {
        enrollment: EnrollmentRef,
    },
}
