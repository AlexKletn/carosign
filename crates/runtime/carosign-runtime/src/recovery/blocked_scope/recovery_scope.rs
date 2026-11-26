use carosign_core::identity::SubjectRef;
use carosign_core::ids::AuthenticatorId;

/// Area of registration affected.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct RecoveryScope {
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
}
