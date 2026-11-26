use crate::identity::SubjectRef;
use crate::ids::AuthenticatorId;
use crate::operation::context::VerificationPurpose;

/// Please check on the named subject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyRequest {
    /// We're checking.
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
    /// Why.
    pub purpose: VerificationPurpose,
}
