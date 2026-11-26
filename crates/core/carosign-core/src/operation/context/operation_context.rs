use super::BootDeadline;
use super::VerificationPurpose;
use crate::device::AuthenticatorRef;
use crate::identity::OwnerRef;
use crate::identity::SubjectRef;
use crate::ids::OperationId;

/// The immutable context of the operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationContext {
    pub operation_id: OperationId,
    /// Subject (allowed by source of personalities).
    pub subject: SubjectRef,
    /// The machine and the tether era.
    pub authenticator: AuthenticatorRef,
    /// Owner and permit.
    pub owner: OwnerRef,
    /// Purpose chosen by a trusted source.
    pub purpose: VerificationPurpose,
    /// Deadline of this download.
    pub deadline: BootDeadline,
}
