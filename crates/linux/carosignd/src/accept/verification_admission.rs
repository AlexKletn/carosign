use super::{
    AcceptError, AuthenticatorId, AuthorityRefusal, CallerContext, GrantScope, MetadataStore,
    PolicyAuthority, SubjectRef, ValidatedGrant, VerificationAuthorityError, VerificationCycle,
    VerificationIntent, authorize_verification, read_candidates, scope_of, select,
};

/// Prepares a cycle of full customer intent and policy resolution.
/// A refunded permit cannot be replaced by a re-calling of the old port.
pub(crate) async fn prepare_client_verification<S: MetadataStore>(
    store: &S,
    identity: &dyn carosign_runtime::ports::IdentityAuthority,
    policy: &dyn PolicyAuthority,
    caller: &CallerContext,
    intent: &VerificationIntent,
    epoch: carosign_core::device::DeviceEpoch,
) -> Result<(VerificationCycle, ValidatedGrant), AcceptError> {
    let (subject, grant) = authorize_client_intent(identity, policy, caller, intent).await?;
    let cycle = prepare_verified_intent(store, caller, intent, epoch, subject, &grant)?;
    Ok((cycle, grant))
}

pub(crate) async fn authorize_client_intent(
    identity: &dyn carosign_runtime::ports::IdentityAuthority,
    policy: &dyn PolicyAuthority,
    caller: &CallerContext,
    intent: &VerificationIntent,
) -> Result<(SubjectRef, ValidatedGrant), AcceptError> {
    authorize_verification(identity, policy, caller, intent)
        .await
        .map_err(|error| match error {
            VerificationAuthorityError::Identity(
                carosign_core::identity::IdentityError::UnknownSubject,
            ) => AcceptError::UnknownSubject,
            VerificationAuthorityError::Identity(
                carosign_core::identity::IdentityError::NotPermitted,
            ) => AcceptError::Unauthorized(AuthorityRefusal::Denied),
            VerificationAuthorityError::Identity(
                carosign_core::identity::IdentityError::Unavailable,
            ) => AcceptError::IdentityUnavailable,
            VerificationAuthorityError::Policy(reason) => AcceptError::Unauthorized(reason),
            VerificationAuthorityError::ScopeMismatch => {
                AcceptError::Unauthorized(AuthorityRefusal::Denied)
            }
        })
}

pub(crate) fn prepare_verified_intent<S: MetadataStore>(
    store: &S,
    caller: &CallerContext,
    intent: &VerificationIntent,
    epoch: carosign_core::device::DeviceEpoch,
    subject: SubjectRef,
    grant: &ValidatedGrant,
) -> Result<VerificationCycle, AcceptError> {
    let scope = scope_of(subject, intent.authenticator);
    let candidates = read_candidates(store, &scope)?;
    let permitted = select(subject, intent.authenticator, epoch, &candidates)?;
    let permitted = if let Some(enrollment) = intent.enrollment {
        let selected = permitted
            .bindings()
            .iter()
            .find(|binding| binding.enrollment == enrollment)
            .ok_or(AcceptError::Scope(super::SelectionError::NotEnrolled))?;
        carosign_runtime::operation::bindings::PermittedSet::new(vec![selected.clone()])
            .map_err(|error| AcceptError::Scope(error.into()))?
    } else {
        permitted
    };
    Ok(VerificationCycle::new(
        caller.clone(),
        grant.scope().clone(),
        permitted,
    ))
}

/// Prepares a verification cycle for the request.
///
/// ⚠️ The order is given by meaning: first the subject (without it there is no area), then a
/// set of registrations (without it there is nothing to check), then the powers.
pub async fn prepare_verification<S, A>(
    store: &S,
    authority: &A,
    caller: &CallerContext,
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    epoch: carosign_core::device::DeviceEpoch,
) -> Result<VerificationCycle, AcceptError>
where
    S: MetadataStore,
    A: PolicyAuthority,
{
    prepare_authorized_verification(store, authority, caller, subject, authenticator, epoch)
        .await
        .map(|(cycle, _grant)| cycle)
}

/// Prepares the check together with the permission issued by the source of the policy.
pub(crate) async fn prepare_authorized_verification<S, A>(
    store: &S,
    authority: &A,
    caller: &CallerContext,
    subject: SubjectRef,
    authenticator: AuthenticatorId,
    epoch: carosign_core::device::DeviceEpoch,
) -> Result<(VerificationCycle, ValidatedGrant), AcceptError>
where
    S: MetadataStore,
    A: PolicyAuthority,
{
    let scope = scope_of(subject, authenticator);
    let candidates = read_candidates(store, &scope)?;

    let permitted = select(subject, authenticator, epoch, &candidates)?;
    let cycle = VerificationCycle::new(
        caller.clone(),
        GrantScope::Verify {
            subject,
            authenticator,
            purpose: carosign_core::operation::context::VerificationPurpose::SessionLogin,
        },
        permitted,
    );
    let grant = authority
        .authorize(&cycle.caller, cycle.scope.clone())
        .await
        .map_err(AcceptError::Unauthorized)?;
    if grant.scope() != &cycle.scope {
        return Err(AcceptError::IdentityUnavailable);
    }
    // During the authorization period, the change could be completed and the blocking
    // removed.
    // Re-read not only the lock, but also the current registrations.
    let candidates = read_candidates(store, &scope)?;
    let permitted = select(subject, authenticator, epoch, &candidates)?;
    Ok((
        VerificationCycle::new(caller.clone(), cycle.scope, permitted),
        grant,
    ))
}
