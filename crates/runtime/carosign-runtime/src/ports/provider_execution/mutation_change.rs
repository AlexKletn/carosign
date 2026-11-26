/// What exactly do we change (Document 34, B-14).
///
/// ⚠️ The difference between “new” and “replacement” is a type variant, not an empty field:
/// `(old=None, desired=None)` In both cases, it is prohibited.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MutationChange {
    /// New Registration: Planned Identity Only
    Create {
        /// New registration and generation (designated by the system).
        planned_new: carosign_core::enrollment::EnrollmentRef,
    },
    /// Replacement: new identity **and* tested old ligament.
    Replace {
        /// New registration and generation.
        planned_new: carosign_core::enrollment::EnrollmentRef,
        /// An old bundle checked on the registry under the area lock.
        old: crate::operation::bindings::EnrollmentBinding,
    },
    /// Removal: **required** with a ligament.
    Removal {
        /// Removable registration and its generation.
        enrollment: carosign_core::enrollment::EnrollmentRef,
        /// A bundle of the same registration (D-07).
        binding: crate::operation::bindings::EnrollmentBinding,
    },
}
