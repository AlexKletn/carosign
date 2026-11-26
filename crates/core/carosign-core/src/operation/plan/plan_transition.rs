use crate::enrollment::EnrollmentRef;

/// The planned transition without ambiguous empty fields.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlanTransition {
    /// Creation of registration.
    Create { planned_new: EnrollmentRef },
    /// Replacement of the exact previous registration.
    Replace {
        old: EnrollmentRef,
        planned_new: EnrollmentRef,
    },
    /// Delete the exact registration.
    Remove { old: EnrollmentRef },
}
