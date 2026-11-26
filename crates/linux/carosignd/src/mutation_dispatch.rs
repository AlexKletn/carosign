//! Allowing a mutation request before an worker starts up or records a log.

use carosign_core::authorization::{GrantScope, ValidatedGrant};
use carosign_core::enrollment::{EnrollmentGeneration, EnrollmentLabel, EnrollmentRef};
use carosign_core::identity::SubjectRef;
use carosign_core::ids::{AuthenticatorId, EnrollmentId, IdempotencyKey};
use carosign_linux_ipc::mapping::client::DomainRequest;
use carosign_persistence::store::MetadataStore;
use carosign_runtime::ports::{CallerContext, PolicyAuthority};

#[cfg(test)]
pub(crate) async fn prepare<S: MetadataStore>(
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    caller: &CallerContext,
    request: &DomainRequest,
    configured_authenticator: AuthenticatorId,
) -> Result<PreparedMutation, MutationPrepareError> {
    let authorized = authorize(policy, caller, request, configured_authenticator).await?;
    prepare_authorized(store, policy, authorized).await
}

pub(crate) async fn authorize(
    policy: &crate::linux_policy::LinuxPolicy,
    caller: &CallerContext,
    request: &DomainRequest,
    configured_authenticator: AuthenticatorId,
) -> Result<AuthorizedMutation, MutationPrepareError> {
    let (idempotency, authenticator, selector, authority_ref, change) = match request {
        DomainRequest::BeginEnrollment {
            idempotency,
            authenticator,
            label,
            subject,
            replace,
            authority_request_ref,
        } => {
            let replace = replace
                .as_ref()
                .map(|value| {
                    let id = EnrollmentId::try_from(value.enrollment_id.as_slice())
                        .map_err(|_| MutationPrepareError::Invalid)?;
                    let target = AuthenticatorId::try_from(value.authenticator_id.as_slice())
                        .map_err(|_| MutationPrepareError::Invalid)?;
                    let generation = EnrollmentGeneration::new(value.generation)
                        .map_err(|_| MutationPrepareError::Invalid)?;
                    if target != *authenticator {
                        return Err(MutationPrepareError::Invalid);
                    }
                    Ok(EnrollmentRef {
                        id,
                        authenticator: target,
                        generation,
                    })
                })
                .transpose()?;
            (
                *idempotency,
                *authenticator,
                subject,
                authority_request_ref,
                MutationRequest::Enroll {
                    label: EnrollmentLabel::new(label)
                        .map_err(|_| MutationPrepareError::Invalid)?,
                    replace,
                },
            )
        }
        DomainRequest::BeginRemoval {
            idempotency,
            authenticator,
            enrollment,
            generation,
            subject,
            authority_request_ref,
        } => {
            let generation = EnrollmentGeneration::new(*generation)
                .map_err(|_| MutationPrepareError::Invalid)?;
            (
                *idempotency,
                *authenticator,
                subject,
                authority_request_ref,
                MutationRequest::Remove {
                    enrollment: EnrollmentRef {
                        id: *enrollment,
                        authenticator: *authenticator,
                        generation,
                    },
                },
            )
        }
        _ => return Err(MutationPrepareError::Invalid),
    };
    if !authority_ref.is_empty() || authenticator != configured_authenticator {
        return Err(MutationPrepareError::Denied);
    }
    let subject =
        policy
            .resolve_mutation_subject(caller, selector)
            .map_err(|error| match error {
                carosign_core::identity::IdentityError::Unavailable => {
                    MutationPrepareError::Unavailable
                }
                _ => MutationPrepareError::Denied,
            })?;
    let scope = match &change {
        MutationRequest::Enroll { replace, .. } => GrantScope::Enroll {
            subject,
            authenticator,
            replace: *replace,
        },
        MutationRequest::Remove { enrollment } => GrantScope::Remove {
            subject,
            enrollment: *enrollment,
        },
    };
    let grant = policy
        .authorize(caller, scope.clone())
        .await
        .map_err(|_| MutationPrepareError::Denied)?;
    if grant.scope() != &scope {
        return Err(MutationPrepareError::Denied);
    }
    Ok(AuthorizedMutation {
        caller: caller.clone(),
        subject,
        authenticator,
        idempotency,
        change,
        grant,
    })
}

pub(crate) async fn prepare_authorized<S: MetadataStore>(
    store: &S,
    policy: &crate::linux_policy::LinuxPolicy,
    authorized: AuthorizedMutation,
) -> Result<PreparedMutation, MutationPrepareError> {
    use carosign_persistence::enrollment_codec::EnrollmentStatus;
    let AuthorizedMutation {
        caller,
        subject,
        authenticator,
        idempotency,
        change,
        grant,
    } = authorized;
    // Only an accurate valid record can be the purpose of replacement or deletion.
    let scope_key = crate::accept::scope_of(subject, authenticator);
    let records = crate::accept::read_enrollment_records(store, &scope_key)
        .map_err(|_| MutationPrepareError::Unavailable)?;
    let target = match &change {
        MutationRequest::Enroll { replace, .. } => *replace,
        MutationRequest::Remove { enrollment } => Some(*enrollment),
    };
    if let Some(target) = target
        && !records
            .iter()
            .any(|record| record.enrollment == target && record.status == EnrollmentStatus::Active)
    {
        return Err(MutationPrepareError::Conflict);
    }
    let scope = carosign_persistence::mutation_plan::Scope {
        subject,
        authenticator,
    };
    let mut bindings: Vec<_> = records
        .iter()
        .map(
            |record| carosign_persistence::mutation_plan::BindingSnapshot {
                enrollment: record.enrollment,
                subject: record.subject,
                kind: record.binding_kind,
                provider_ref_digest: carosign_persistence::mutation_plan::sha256(
                    record.provider_ref.as_bytes(),
                ),
                format_id: record.format_id.clone(),
                format_version: record.format_version,
            },
        )
        .collect();
    bindings.sort_by_key(|binding| (binding.enrollment.id, binding.enrollment.generation));
    use carosign_persistence::mutation_plan::{MutationPlan, Transition};
    let plan = match &change {
        MutationRequest::Enroll { label, replace } => {
            let id = EnrollmentId::from_bytes(
                carosign_linux::sys::random_16().map_err(|_| MutationPrepareError::Unavailable)?,
            )
            .map_err(|_| MutationPrepareError::Unavailable)?;
            let generation = match replace {
                Some(old) => old
                    .generation
                    .next()
                    .map_err(|_| MutationPrepareError::Conflict)?,
                None => EnrollmentGeneration::FIRST,
            };
            MutationPlan::build(
                if replace.is_some() {
                    Transition::EnrollReplace
                } else {
                    Transition::EnrollNew
                },
                *replace,
                Some(id),
                generation.value(),
                bindings,
                scope,
            )
            .and_then(|plan| plan.with_requested_label(scope, label.as_str()))
        }
        MutationRequest::Remove { enrollment } => MutationPlan::build(
            Transition::Remove,
            Some(*enrollment),
            None,
            0,
            bindings,
            scope,
        ),
    }
    .map_err(|_| MutationPrepareError::Conflict)?;
    let request_digest = carosign_persistence::mutation_plan::canonical_request_digest(
        scope,
        plan.transition,
        plan.old_target,
        plan.requested_label.as_deref(),
    )
    .map_err(|_| MutationPrepareError::Invalid)?;
    policy
        .revalidate_caller(
            &caller,
            &grant,
            carosign_core::authorization::AuthorizationBoundary::BeforeDispatch,
        )
        .await
        .map_err(|_| MutationPrepareError::Denied)?;
    Ok(PreparedMutation {
        caller,
        subject,
        authenticator,
        idempotency,
        request: change,
        grant,
        plan,
        request_digest,
    })
}

#[path = "mutation_dispatch.tests.rs"]
#[cfg(test)]
mod tests;

mod mutation_request;
pub(crate) use mutation_request::MutationRequest;
mod prepared_mutation;
pub(crate) use prepared_mutation::PreparedMutation;
mod mutation_prepare_error;
pub(crate) use mutation_prepare_error::MutationPrepareError;

mod authorized_mutation;
pub(crate) use authorized_mutation::AuthorizedMutation;
