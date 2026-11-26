use super::{authenticator_list::format_authenticators, transport_error::map_error};
use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::client::v1::{self as wire, client_frame::Payload};
use carosign_protocol::carosign::common::v1 as common;
use carosignctl::client::ClientError;

pub(super) fn subject(user: Option<&str>) -> common::SubjectSelector {
    common::SubjectSelector {
        selector: Some(match user {
            Some(user) => common::subject_selector::Selector::AccountHint(user.to_owned()),
            None => common::subject_selector::Selector::CurrentSubject(true),
        }),
    }
}

pub(super) fn devices(
    transport: &Transport,
) -> Result<Vec<common::AuthenticatorDescriptor>, ClientError> {
    let answer = transport
        .request(Payload::ListAuthenticators(wire::ListAuthenticators {}))
        .map_err(map_error)?;
    let Some(Payload::Authenticators(devices)) = answer.payload else {
        return Err(ClientError::Unavailable);
    };
    format_authenticators(devices.clone())?;
    Ok(devices.entries)
}

pub(super) fn enrollments(
    transport: &Transport,
    user: Option<&str>,
    id: &[u8],
) -> Result<wire::Enrollments, ClientError> {
    let answer = transport
        .request(Payload::ListEnrollments(wire::ListEnrollments {
            subject: Some(subject(user)),
            authenticator_id: id.to_vec(),
            authority_request_ref: Vec::new(),
        }))
        .map_err(map_error)?;
    match answer.payload {
        Some(Payload::Enrollments(items))
            if items.entries.len() <= carosign_protocol::limits::ENROLLMENTS_PER_RESPONSE_MAX =>
        {
            Ok(items)
        }
        Some(Payload::ProtocolError(_)) => Err(ClientError::Refused {
            message: "запрос отклонён службой".into(),
        }),
        _ => Err(ClientError::Unexpected),
    }
}

pub(super) fn list_enrollments(path: &str, user: Option<&str>) -> Result<Vec<String>, ClientError> {
    let transport = Transport::connect_root(path).map_err(map_error)?;
    list_enrollments_connected(&transport, user)
}

pub(super) fn list_enrollments_connected(
    transport: &Transport,
    user: Option<&str>,
) -> Result<Vec<String>, ClientError> {
    let devices = devices(transport)?;
    let mut lines = Vec::new();
    for device in devices {
        let entries = enrollments(transport, user, &device.authenticator_id)?;
        for item in entries.entries {
            let enrollment = item.enrollment.ok_or(ClientError::Unexpected)?;
            if enrollment.enrollment_id.len() != 16
                || enrollment.enrollment_id.iter().all(|v| *v == 0)
                || enrollment.authenticator_id != device.authenticator_id
                || enrollment.generation == 0
            {
                return Err(ClientError::Unexpected);
            }
            let id = enrollment
                .enrollment_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            lines.push(format!(
                "{}  {}  {}{}",
                carosignctl::output::short(
                    device
                        .authenticator_id
                        .as_slice()
                        .try_into()
                        .map_err(|_| ClientError::Unexpected)?
                ),
                id,
                item.label,
                if item.pending_reconciliation {
                    "  нужна сверка"
                } else {
                    ""
                }
            ));
        }
    }
    if lines.is_empty() {
        lines.push("регистраций нет".into());
    }
    Ok(lines)
}
