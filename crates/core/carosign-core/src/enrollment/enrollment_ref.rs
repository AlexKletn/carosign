use super::EnrollmentGeneration;
use crate::ids::AuthenticatorId;
use crate::ids::EnrollmentId;
use core::fmt;

/// Reference to registration: identifier, apparatus, and **fixed generation**.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnrollmentRef {
    /// Registry.
    pub id: EnrollmentId,
    /// The device in which the registration is located.
    pub authenticator: AuthenticatorId,
    /// Generation at the time of exile.
    pub generation: EnrollmentGeneration,
}

impl fmt::Debug for EnrollmentRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EnrollmentRef(.., generation={})",
            self.generation.value()
        )
    }
}
