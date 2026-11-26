use carosign_core::{
    authorization::GrantScope,
    enrollment::{EnrollmentGeneration, EnrollmentLabel, EnrollmentRef},
    ids::{AuthenticatorId, EnrollmentId},
};
use carosign_persistence::{model::StorageError, store::MetadataStore};
use carosign_protocol::carosign::{
    client::v1::{EnrollmentRenamed, RenameEnrollment},
    common::v1::{ErrorCode, SubjectSelector, subject_selector::Selector},
};
use carosign_runtime::ports::PolicyAuthority;

use crate::{linux_policy::LinuxPolicy, server::Connection};

pub(crate) async fn rename_enrollment(
    store: &impl MetadataStore,
    policy: &LinuxPolicy,
    connection: &Connection,
    request: &RenameEnrollment,
) -> Result<EnrollmentRenamed, ErrorCode> {
    let invalid = ErrorCode::ErrorInvalidMessage;
    let denied = ErrorCode::ErrorUnauthorized;
    let reference = request.enrollment.as_ref().ok_or(invalid)?;
    let target = EnrollmentRef {
        authenticator: AuthenticatorId::try_from(reference.authenticator_id.as_slice())
            .map_err(|_| invalid)?,
        id: EnrollmentId::try_from(reference.enrollment_id.as_slice()).map_err(|_| invalid)?,
        generation: EnrollmentGeneration::new(reference.generation).map_err(|_| invalid)?,
    };
    let expected = EnrollmentLabel::new(request.expected_label.as_deref().ok_or(invalid)?)
        .map_err(|_| invalid)?;
    let label = EnrollmentLabel::new(&request.label).map_err(|_| invalid)?;
    let caller = crate::cycle::caller_from_session(&connection.session).ok_or(denied)?;
    let subject = policy
        .resolve_mutation_subject(
            &caller,
            &SubjectSelector {
                selector: Some(Selector::CurrentSubject(true)),
            },
        )
        .map_err(|_| denied)?;
    let scope = GrantScope::Enroll {
        subject,
        authenticator: target.authenticator,
        replace: None,
    };
    let grant = policy
        .authorize(&caller, scope.clone())
        .await
        .map_err(|_| denied)?;
    if grant.scope() != &scope {
        return Err(denied);
    }
    let mut authorization_error = None;
    carosign_persistence::mutation_store::rename_enrollment(
        store,
        subject,
        target,
        &expected,
        &label,
        || {
            policy.revalidate_now(&caller, &grant).map_err(|_| {
                authorization_error = Some(ErrorCode::ErrorGrantRevoked);
                StorageError::Unavailable
            })
        },
    )
    .map_err(|error| {
        authorization_error.unwrap_or(match error {
            StorageError::GenerationConflict => ErrorCode::ErrorGenerationConflict,
            StorageError::Corrupt { .. } => ErrorCode::ErrorCorruptState,
            _ => ErrorCode::ErrorStoreUnavailable,
        })
    })?;
    Ok(EnrollmentRenamed {
        enrollment: Some(reference.clone()),
        label: request.label.clone(),
    })
}
