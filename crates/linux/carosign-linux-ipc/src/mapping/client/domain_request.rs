use super::MappingError;

use carosign_core::ids::{AuthenticatorId, IdempotencyKey, OperationId};
use carosign_protocol::carosign::common::v1 as common;

use crate::codec::ClientRequest;

/// A client's domain request.
#[derive(Clone, PartialEq)]
pub enum DomainRequest {
    ListAuthenticators,
    BeginVerification {
        /// Key.
        idempotency: IdempotencyKey,
        authenticator: AuthenticatorId,
        /// Untested choice of subject.
        subject: common::SubjectSelector,
        /// Unverified target clue.
        purpose_hint: common::VerificationPurpose,
        enrollment: Option<common::EnrollmentRef>,
        report_matched_enrollment: bool,
        /// An unverified reference to the source of authority.
        authority_request_ref: Vec<u8>,
    },
    BeginEnrollment {
        /// Key.
        idempotency: IdempotencyKey,
        authenticator: AuthenticatorId,
        /// Label (already limited).
        label: String,
        /// Untested choice of subject.
        subject: common::SubjectSelector,
        /// Registration requested to be replaced.
        replace: Option<common::EnrollmentRef>,
        /// An unverified reference to the source of authority.
        authority_request_ref: Vec<u8>,
    },
    /// Start deleting.
    BeginRemoval {
        /// Key.
        idempotency: IdempotencyKey,
        authenticator: AuthenticatorId,
        enrollment: carosign_core::ids::EnrollmentId,
        generation: u64,
        /// Untested choice of subject.
        subject: common::SubjectSelector,
        /// An unverified reference to the source of authority.
        authority_request_ref: Vec<u8>,
    },
    /// To watch.
    Observe {
        operation: OperationId,
        /// Repeat events after this number.
        after_sequence: u64,
    },
    /// Ask.
    Get {
        operation: OperationId,
    },
    Cancel {
        operation: OperationId,
    },
    Health,
}

impl core::fmt::Debug for DomainRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::ListAuthenticators => "ListAuthenticators",
            Self::BeginVerification { .. } => "BeginVerification(..)",
            Self::BeginEnrollment { .. } => "BeginEnrollment(..)",
            Self::BeginRemoval { .. } => "BeginRemoval(..)",
            Self::Observe { .. } => "Observe(..)",
            Self::Get { .. } => "Get(..)",
            Self::Cancel { .. } => "Cancel(..)",
            Self::Health => "Health",
        })
    }
}

fn authenticator(bytes: &[u8]) -> Result<AuthenticatorId, MappingError> {
    AuthenticatorId::try_from(bytes).map_err(|_| MappingError::BadId)
}

fn key(bytes: &[u8]) -> Result<IdempotencyKey, MappingError> {
    IdempotencyKey::try_from(bytes).map_err(|_| MappingError::BadId)
}

fn operation(bytes: &[u8]) -> Result<OperationId, MappingError> {
    OperationId::try_from(bytes).map_err(|_| MappingError::BadId)
}

/// Translates the disassembled request to the domain.
pub fn to_domain(request: &ClientRequest) -> Result<Option<DomainRequest>, MappingError> {
    Ok(match request {
        ClientRequest::Hello { .. } | ClientRequest::RenameEnrollment(_) => None,
        ClientRequest::ListAuthenticators => Some(DomainRequest::ListAuthenticators),
        ClientRequest::GetHealth => Some(DomainRequest::Health),
        ClientRequest::GetAuthenticator { authenticator: id } => {
            let _ = authenticator(id)?;
            None
        }
        ClientRequest::ListEnrollments {
            authenticator: id, ..
        } => {
            let _ = authenticator(id)?;
            None
        }
        ClientRequest::BeginVerification {
            idempotency_key,
            authenticator: id,
            subject,
            purpose_hint,
            enrollment,
            report_matched_enrollment,
            authority_request_ref,
        } => Some(DomainRequest::BeginVerification {
            enrollment: enrollment.clone(),
            report_matched_enrollment: *report_matched_enrollment,
            idempotency: key(idempotency_key)?,
            authenticator: authenticator(id)?,
            subject: subject.clone(),
            purpose_hint: *purpose_hint,
            authority_request_ref: authority_request_ref.clone(),
        }),
        ClientRequest::BeginEnrollment {
            idempotency_key,
            authenticator: id,
            label,
            subject,
            replace,
            authority_request_ref,
        } => Some(DomainRequest::BeginEnrollment {
            idempotency: key(idempotency_key)?,
            authenticator: authenticator(id)?,
            label: label.clone(),
            subject: subject.clone(),
            replace: replace.clone(),
            authority_request_ref: authority_request_ref.clone(),
        }),
        ClientRequest::BeginRemoval {
            idempotency_key,
            authenticator: id,
            enrollment,
            generation,
            subject,
            authority_request_ref,
        } => Some(DomainRequest::BeginRemoval {
            idempotency: key(idempotency_key)?,
            authenticator: authenticator(id)?,
            enrollment: carosign_core::ids::EnrollmentId::try_from(&enrollment[..])
                .map_err(|_| MappingError::BadId)?,
            generation: *generation,
            subject: subject.clone(),
            authority_request_ref: authority_request_ref.clone(),
        }),
        ClientRequest::ObserveOperation {
            operation: id,
            after_sequence,
        } => Some(DomainRequest::Observe {
            operation: operation(id)?,
            after_sequence: *after_sequence,
        }),
        ClientRequest::GetOperation { operation: id } => Some(DomainRequest::Get {
            operation: operation(id)?,
        }),
        ClientRequest::CancelOperation { operation: id } => Some(DomainRequest::Cancel {
            operation: operation(id)?,
        }),
    })
}

#[path = "domain_request.tests.rs"]
#[cfg(test)]
mod tests;
