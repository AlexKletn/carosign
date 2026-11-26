use super::{
    AuthenticatorId, CallerContext, IdempotencyKey, MutationRequest, SubjectRef, ValidatedGrant,
};

/// The result of the check is the entrance to create the actor, but not the right to send.
pub(crate) struct PreparedMutation {
    pub(crate) caller: CallerContext,
    pub(crate) subject: SubjectRef,
    pub(crate) authenticator: AuthenticatorId,
    pub(crate) idempotency: IdempotencyKey,
    pub(crate) request: MutationRequest,
    pub(crate) grant: ValidatedGrant,
    pub(crate) plan: carosign_persistence::mutation_plan::MutationPlan,
    pub(crate) request_digest: [u8; 32],
}
