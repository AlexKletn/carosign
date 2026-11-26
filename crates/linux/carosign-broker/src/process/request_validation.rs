use carosign_protocol::carosign::{
    broker_relay::v1::{MutationPermit, Scope},
    provider::v1::ProviderFrame,
};

use super::Context;

pub(super) fn valid_id(bytes: &[u8]) -> bool {
    bytes.len() == 16 && bytes.iter().any(|b| *b != 0)
}

pub(super) fn checked_scope(scope: &Scope, context: &Context) -> bool {
    let Some(owner) = scope.owner.as_ref() else {
        return false;
    };
    valid_id(&scope.lease_id)
        && valid_id(&owner.operation_id)
        && valid_id(&owner.authenticator_id)
        && owner.worker_instance == context.worker
        && owner.device_epoch == context.epoch
        && !scope.claims.is_empty()
        && scope.claims.len() <= 8
        && scope.claims.iter().enumerate().all(|(i, claim)| {
            valid_id(&claim.resource_id)
                && (1..=3).contains(&claim.mode)
                && (i == 0 || scope.claims[i - 1].resource_id < claim.resource_id)
        })
}

pub(super) fn valid_mutation(
    context: &Context,
    scope: &Scope,
    permit: &MutationPermit,
    frame: &ProviderFrame,
    mutation: &[u8],
    subject: Option<&carosign_protocol::carosign::common::v1::SubjectRef>,
    authenticator: Option<&carosign_protocol::carosign::common::v1::AuthenticatorRef>,
) -> bool {
    let (Some(owner), Some(subject), Some(authenticator)) =
        (scope.owner.as_ref(), subject, authenticator)
    else {
        return false;
    };
    checked_scope(scope, context)
        && scope.claims.iter().all(|claim| claim.mode == 1)
        && permit.lease_id == scope.lease_id
        && permit.scope.len() == 48
        && valid_id(&subject.authority_id)
        && valid_id(&subject.subject_id)
        && permit.scope[..16] == subject.authority_id
        && permit.scope[16..32] == subject.subject_id
        && permit.scope[32..] == owner.authenticator_id
        && valid_id(mutation)
        && permit.mutation_id == mutation
        && frame.operation_id == owner.operation_id
        && valid_id(&frame.command_id)
        && authenticator.authenticator_id == owner.authenticator_id
        && authenticator.device_epoch == context.epoch
}

#[cfg(test)]
#[path = "request_validation.tests.rs"]
mod tests;
