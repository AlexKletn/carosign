use super::Request;
use super::select_for_account;
use super::verification_request;
use super::{Observation, observe_verification_with_acceptance};
use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::client::v1::BeginVerification;
use carosign_protocol::carosign::client::v1::client_frame::Payload;
use carosign_protocol::carosign::common::v1 as common;

/// It accesses the system service through the root socket, selects the device, and observes
/// the outcome. The service is required to authorize the name and purpose for the client; the
/// call itself does not issue PAM permission.
pub fn observe_system_verification(
    path: &str,
    request: &Request,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    observe_system_verification_for_modality(path, request, idempotency_key, purpose, None)
}

/// Notifies the PAM owner when the service has accepted a specific check.
pub fn observe_system_verification_with_acceptance(
    path: &str,
    request: &Request,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
    accepted: impl FnOnce(&[u8]),
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    observe_system_verification_inner(path, request, idempotency_key, purpose, None, |_, id| {
        accepted(id)
    })
}

/// Transfers the same connection to the owner of the check to cancel the accepted operation.
pub fn observe_system_verification_with_control(
    path: &str,
    request: &Request,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
    accepted: impl FnOnce(
        std::sync::Arc<Transport>,
        &carosign_protocol::carosign::client::v1::OperationAccepted,
    ),
    progress: impl FnMut(carosign_protocol::carosign::client::v1::EventKind),
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    observe_system_verification_inner_with_progress(
        path,
        request,
        idempotency_key,
        purpose,
        None,
        accepted,
        progress,
    )
}

/// Performs check with a limited view of the device if it is required by an external
/// interface, such as fprintd.
pub fn observe_system_verification_for_modality(
    path: &str,
    request: &Request,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
    modality: Option<common::Modality>,
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    observe_system_verification_inner(path, request, idempotency_key, purpose, modality, |_, _| {})
}

fn observe_system_verification_inner(
    path: &str,
    request: &Request,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
    modality: Option<common::Modality>,
    accepted: impl FnOnce(std::sync::Arc<Transport>, &[u8]),
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    observe_system_verification_inner_with_progress(
        path,
        request,
        idempotency_key,
        purpose,
        modality,
        |transport, operation| accepted(transport, &operation.operation_id),
        |_| {},
    )
}

fn observe_system_verification_inner_with_progress(
    path: &str,
    request: &Request,
    idempotency_key: [u8; 16],
    purpose: common::VerificationPurpose,
    modality: Option<common::Modality>,
    accepted: impl FnOnce(
        std::sync::Arc<Transport>,
        &carosign_protocol::carosign::client::v1::OperationAccepted,
    ),
    progress: impl FnMut(carosign_protocol::carosign::client::v1::EventKind),
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    use carosign_linux_ipc::client::transport::TransportError;
    request.validate().map_err(|_| TransportError::Unexpected)?;
    let transport = std::sync::Arc::new(Transport::connect_root(path)?);
    let response = transport.request(Payload::ListAuthenticators(
        carosign_protocol::carosign::client::v1::ListAuthenticators {},
    ))?;
    let Some(Payload::Authenticators(authenticators)) = response.payload else {
        return Err(TransportError::Unavailable);
    };
    let eligible: Vec<_> = authenticators
        .entries
        .into_iter()
        .filter(|entry| {
            modality.is_none_or(|m| {
                entry
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.modality == m as i32)
            })
        })
        .collect();
    let authenticator = select_for_account(&eligible, |id| {
        let response = transport.request(Payload::ListEnrollments(
            carosign_protocol::carosign::client::v1::ListEnrollments {
                subject: Some(common::SubjectSelector {
                    selector: Some(common::subject_selector::Selector::AccountHint(
                        request.user.clone(),
                    )),
                }),
                authenticator_id: id.to_vec(),
                authority_request_ref: Vec::new(),
            },
        ))?;
        let Some(Payload::Enrollments(enrollments)) = response.payload else {
            return Err(TransportError::Unavailable);
        };
        Ok(enrollments
            .entries
            .iter()
            .any(|entry| !entry.pending_reconciliation && entry.enrollment.is_some()))
    })?
    .ok_or(TransportError::Unavailable)?;
    let verification = verification_request(request, authenticator, idempotency_key, purpose)
        .map_err(|_| TransportError::Unexpected)?;
    carosign_linux_ipc::client::verification_events::observe_verification_with_progress(
        &transport,
        verification,
        |id| accepted(transport.clone(), id),
        progress,
    )
}

/// Performs one check on a new connection and closes it when it exits.
///
/// This is a connection lifetime limit for a single PAM call over B-04. The result remains
/// untrusted: service authentication, policy, and call-related confirmation are not performed
/// here.
pub fn observe_on_new_connection(
    path: &str,
    request: BeginVerification,
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    let transport = Transport::connect(path)?;
    observe_verification(&transport, request)
}

/// The result is exactly one operation, and the link and the query parameters remain
/// unconfirmed until the system authority owner checks.
pub fn observe_verification(
    transport: &Transport,
    request: BeginVerification,
) -> Result<Observation, carosign_linux_ipc::client::transport::TransportError> {
    observe_verification_with_acceptance(transport, request, |_| {})
}
