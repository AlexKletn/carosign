use super::{OperationActor, ValidatedGrant};

/// Verifies the description of the permit with the operation; the policy checks its
/// credibility.
pub(crate) fn grant_matches_operation(actor: &OperationActor, grant: &ValidatedGrant) -> bool {
    let context = &actor.state().common.context;
    grant.scope() == &actor.state().required_grant_scope()
        && grant.id() == context.owner.binding
        && grant.issuer() == context.owner.authority
        && grant.boot() == context.deadline.boot()
}
