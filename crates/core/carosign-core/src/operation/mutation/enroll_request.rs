use crate::enrollment::EnrollmentLabel;
use crate::enrollment::EnrollmentRef;
use crate::identity::SubjectRef;
use crate::ids::AuthenticatorId;

/// Please register.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnrollRequest {
    /// Who we're registering.
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
    /// A name for a client.
    pub target: EnrollmentLabel,
    /// Replacement of the existing generation by CAS only.
    pub replace: Option<EnrollmentRef>,
}
