use crate::enrollment_authority::AuthorityChannel;
use carosign_core::authorization::ValidatedGrant;
use carosign_core::device::AuthenticatorRef;
use carosign_runtime::enrollment_authorization::EnrollmentAuthorizationError as Error;
use carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope;
use carosign_runtime::enrollment_authorization::EnrollmentRecipientId;

/// The verified line-up finds the recipient and transmits two independent closed channels to
/// one trusted backend. SO_PEERCRED It does not prove the origin of the backend or the
/// authorization; the launcher is responsible for that, too.
pub trait TrustedEnrollmentConnector: Send + Sync {
    fn recipient_for(
        &self,
        authenticator: AuthenticatorRef,
    ) -> Result<EnrollmentRecipientId, Error>;
    fn connect(
        &self,
        scope: &EnrollmentAuthorizationScope,
        grant: &ValidatedGrant,
    ) -> Result<(AuthorityChannel, AuthorityChannel), Error>;
}
