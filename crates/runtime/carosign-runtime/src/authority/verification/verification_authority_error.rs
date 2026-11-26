use crate::ports::AuthorityRefusal;
use carosign_core::identity::IdentityError;

/// Refusal at the stage of obtaining authority.
#[derive(Debug)]
pub enum VerificationAuthorityError {
    /// Rejection of identity source.
    Identity(IdentityError),
    /// Refusal of policy source, including recall and expiration.
    Policy(AuthorityRefusal),
    /// The source returned permission for another entity, apparatus, or operation.
    ScopeMismatch,
}
