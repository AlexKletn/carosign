use crate::generated::carosign::client::v1 as client;
use crate::generated::carosign::common::v1 as common;
use crate::limits;

use super::{WireError, bounded, bounded_str, check_version, known_enum, optional_id, require_id};

/// Checking the client's frame.
pub fn check_client_frame(frame: &client::ClientFrame) -> Result<(), WireError> {
    check_version(frame.major, frame.minor)?;
    require_id(&frame.request_id, "request_id")?;
    optional_id(&frame.operation_id, "operation_id")?;

    if let Some(payload) = &frame.payload {
        check_client_payload(payload)?;
    }
    Ok(())
}

fn check_client_payload(payload: &client::client_frame::Payload) -> Result<(), WireError> {
    use client::client_frame::Payload as P;
    match payload {
        P::Hello(hello) => {
            check_version(hello.major, hello.min_minor)?;
            if hello.min_minor > hello.max_minor {
                return Err(WireError::UnsupportedVersion {
                    major: hello.major,
                    minor: hello.min_minor,
                });
            }
        }
        P::ListEnrollments(request) => {
            check_required_subject(request.subject.as_ref())?;
            require_id(&request.authenticator_id, "authenticator_id")?;
            bounded(
                &request.authority_request_ref,
                "authority_request_ref",
                limits::AUTHORITY_REQUEST_REF_MAX,
            )?;
        }
        P::RenameEnrollment(request) => {
            check_enrollment_ref(request.enrollment.as_ref().ok_or(WireError::BadId {
                field: "enrollment",
            })?)?;
            bounded_str(&request.label, "label", limits::LABEL_MAX)?;
            bounded_str(request.expected_label.as_deref().ok_or(WireError::UnexpectedPayload {
                what: "missing expected enrollment label",
            })?, "expected_label", limits::LABEL_MAX)?;
        }
        P::EnrollmentRenamed(response) => {
            check_enrollment_ref(response.enrollment.as_ref().ok_or(WireError::BadId {
                field: "enrollment",
            })?)?;
            bounded_str(&response.label, "label", limits::LABEL_MAX)?;
        }
        P::BeginVerification(request) => {
            if let Some(enrollment) = &request.enrollment {
                check_enrollment_ref(enrollment)?;
                if enrollment.authenticator_id != request.authenticator_id {
                    return Err(WireError::UnexpectedPayload {
                        what: "verification enrollment authenticator differs",
                    });
                }
            }
            check_required_subject(request.subject.as_ref())?;
            require_id(&request.idempotency_key, "idempotency_key")?;
            require_id(&request.authenticator_id, "authenticator_id")?;
            known_enum(
                request.purpose_hint,
                "purpose_hint",
                &[(1, ()), (2, ()), (3, ()), (4, ()), (5, ())],
            )?;
            bounded(
                &request.authority_request_ref,
                "authority_request_ref",
                limits::AUTHORITY_REQUEST_REF_MAX,
            )?;
        }
        P::BeginEnrollment(request) => {
            check_required_subject(request.subject.as_ref())?;
            require_id(&request.idempotency_key, "idempotency_key")?;
            require_id(&request.authenticator_id, "authenticator_id")?;
            bounded_str(&request.label, "label", limits::LABEL_MAX)?;
            if let Some(replace) = &request.replace {
                check_enrollment_ref(replace)?;
                if replace.authenticator_id != request.authenticator_id {
                    return Err(WireError::UnexpectedPayload {
                        what: "аутентификатор замены не совпадает с запросом",
                    });
                }
            }
            bounded(
                &request.authority_request_ref,
                "authority_request_ref",
                limits::AUTHORITY_REQUEST_REF_MAX,
            )?;
        }
        P::BeginRemoval(request) => {
            check_required_subject(request.subject.as_ref())?;
            require_id(&request.idempotency_key, "idempotency_key")?;
            bounded(
                &request.authority_request_ref,
                "authority_request_ref",
                limits::AUTHORITY_REQUEST_REF_MAX,
            )?;
            let Some(enrollment) = &request.enrollment else {
                return Err(WireError::BadId {
                    field: "enrollment",
                });
            };
            check_enrollment_ref(enrollment)?;
        }
        P::ObserveOperation(request) => {
            require_id(&request.operation_id, "operation_id")?;
        }
        P::GetOperation(request) => {
            require_id(&request.operation_id, "operation_id")?;
        }
        P::CancelOperation(request) => {
            require_id(&request.operation_id, "operation_id")?;
        }
        P::GetAuthenticator(request) => {
            require_id(&request.authenticator_id, "authenticator_id")?;
        }
        P::Health(_) | P::ListAuthenticators(_) | P::GetHealth(_) => {}
        _ => {}
    }
    Ok(())
}

fn check_required_subject(selector: Option<&common::SubjectSelector>) -> Result<(), WireError> {
    check_subject_selector(selector.ok_or(WireError::UnexpectedPayload {
        what: "отсутствует выбор субъекта",
    })?)
}

fn check_enrollment_ref(enrollment: &common::EnrollmentRef) -> Result<(), WireError> {
    require_id(&enrollment.enrollment_id, "enrollment_id")?;
    require_id(&enrollment.authenticator_id, "authenticator_id")?;
    if enrollment.generation == 0 {
        return Err(WireError::BadId {
            field: "generation",
        });
    }
    Ok(())
}

/// Testing the subject's selection.
pub fn check_subject_selector(selector: &common::SubjectSelector) -> Result<(), WireError> {
    use common::subject_selector::Selector;
    match &selector.selector {
        Some(Selector::CurrentSubject(current)) => {
            if !*current {
                return Err(WireError::UnexpectedPayload {
                    what: "current_subject=false",
                });
            }
        }
        Some(Selector::AccountHint(hint)) => {
            bounded_str(hint, "account_hint", limits::LABEL_MAX)?;
        }
        Some(Selector::OpaqueAuthoritySelector(bytes)) => {
            if bytes.is_empty() || bytes.len() > limits::OPAQUE_SELECTOR_MAX {
                return Err(WireError::FieldTooLong {
                    field: "opaque_authority_selector",
                    actual: bytes.len(),
                    allowed: limits::OPAQUE_SELECTOR_MAX,
                });
            }
        }
        None => {
            return Err(WireError::UnexpectedPayload {
                what: "пустой выбор субъекта",
            });
        }
    }
    Ok(())
}
