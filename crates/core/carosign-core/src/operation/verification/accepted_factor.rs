use crate::enrollment::EnrollmentRef;
use core::fmt;

/// I got it back from the self-test.
///
/// ⚠️ A separate type of result, so that the “self-check passed” could not be passed off as
/// an entrance: it has its own form and its purpose.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AcceptedFactor {
    enrollment: EnrollmentRef,
}

impl AcceptedFactor {
    /// The accepted factor is for specific registration only.
    pub fn for_enrollment(enrollment: EnrollmentRef) -> Self {
        Self { enrollment }
    }

    /// Which registration is referenced.
    pub fn enrollment(&self) -> EnrollmentRef {
        self.enrollment
    }
}

impl fmt::Debug for AcceptedFactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcceptedFactor(..)")
    }
}
