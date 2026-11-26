use carosign_core::{
    authorization::ValidatedGrant,
    identity::SubjectRef,
    ids::{AuthenticatorId, IdempotencyKey},
};
use carosign_runtime::ports::CallerContext;

use super::MutationRequest;

pub(crate) struct AuthorizedMutation {
    pub(crate) caller: CallerContext,
    pub(crate) subject: SubjectRef,
    pub(crate) authenticator: AuthenticatorId,
    pub(crate) idempotency: IdempotencyKey,
    pub(crate) change: MutationRequest,
    pub(crate) grant: ValidatedGrant,
}
