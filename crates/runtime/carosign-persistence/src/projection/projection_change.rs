use super::{EnrollmentBinding, EnrollmentRef};

/// Proven projection of change.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProjectionChange {
    /// New registration.
    Create {
        /// A new identity.
        planned_new: EnrollmentRef,
    },
    /// Replacement: A new identity and a proven old ligament.
    Replace {
        /// A new identity.
        planned_new: EnrollmentRef,
        /// Old bundle.
        old: EnrollmentBinding,
    },
}
