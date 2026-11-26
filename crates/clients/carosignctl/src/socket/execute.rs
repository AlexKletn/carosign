use super::{
    authenticator_list::format_authenticators, inventory::list_enrollments, mutation,
    operation_id::parse_operation_id, operation_status::format_snapshot,
    transport_error::map_error, verification::verify,
};
use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::client::v1::{self as wire, client_frame::Payload};
use carosignctl::client::ClientError;
use carosignctl::commands::Command;

pub(crate) fn execute(command: &Command, path: &str) -> Result<Vec<String>, ClientError> {
    match command {
        Command::ListEnrollments { user } => return list_enrollments(path, user.as_deref()),
        Command::Verify { user } => return verify(path, user),
        Command::Enroll {
            user,
            label,
            device,
        } => return mutation::enroll(path, user, label, device.as_deref()),
        Command::Remove { enrollment, user } => {
            return mutation::remove(path, enrollment, user.as_deref());
        }
        _ => {}
    }
    let operation = match command {
        Command::Status { operation } | Command::Cancel { operation } => {
            Some(parse_operation_id(operation)?)
        }
        _ => None,
    };
    let payload = match command {
        Command::Health => Payload::GetHealth(wire::GetHealth {}),
        Command::ListAuthenticators => Payload::ListAuthenticators(wire::ListAuthenticators {}),
        Command::Status { .. } => Payload::GetOperation(wire::GetOperation {
            operation_id: operation.expect("идентификатор разобран").to_vec(),
        }),
        Command::Cancel { .. } => Payload::CancelOperation(wire::CancelOperation {
            operation_id: operation.expect("идентификатор разобран").to_vec(),
        }),
        _ => return Err(ClientError::Unavailable),
    };
    let expect_health = matches!(payload, Payload::GetHealth(_));
    let transport = Transport::connect(path).map_err(map_error)?;
    let answer = transport.request(payload).map_err(map_error)?;
    match answer.payload {
        Some(Payload::Health(h)) if expect_health => Ok(vec![format!(
            "служба: {}; аутентификаторы доступны: {}, недоступны: {}; областей для сверки: {}",
            if h.service_ready {
                "готова"
            } else {
                "не готова"
            },
            h.available_authenticators,
            h.unavailable_authenticators,
            h.scopes_needing_reconciliation
        )]),
        Some(Payload::Authenticators(a)) if matches!(command, Command::ListAuthenticators) => {
            format_authenticators(a)
        }
        Some(Payload::OperationSnapshot(snapshot)) if matches!(command, Command::Status { .. }) => {
            format_snapshot(snapshot, operation.expect("идентификатор разобран"))
        }
        Some(Payload::CancellationReceipt(receipt))
            if matches!(command, Command::Cancel { .. }) =>
        {
            match wire::CancelReceiptKind::from_i32(receipt.result) {
                Some(wire::CancelReceiptKind::CancelReceiptRequested) => {
                    Ok(vec!["отмена запрошена".into()])
                }
                Some(wire::CancelReceiptKind::CancelReceiptAlreadyTerminal) => {
                    Ok(vec!["операция уже завершена".into()])
                }
                _ => Err(ClientError::Unexpected),
            }
        }
        Some(Payload::ProtocolError(_)) => Err(ClientError::Refused {
            message: "запрос отклонён службой".into(),
        }),
        _ => Err(ClientError::Unexpected),
    }
}
