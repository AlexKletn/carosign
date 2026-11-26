use carosign_core::ids::AuthenticatorId;
use carosign_linux_ipc::codec::{ClientRequest, error_frame};
use carosign_protocol::carosign::client::v1::{Authenticator, ClientFrame, client_frame::Payload};
use carosign_protocol::carosign::common::v1::ErrorCode;
use carosign_runtime::ports::AuthorityRefusal;
use prost::Message;

use super::{Connection, Ended, VerificationDispatcher};

pub(super) fn answer_inventory(
    connection: &Connection,
    dispatcher: &dyn VerificationDispatcher,
    request: &ClientRequest,
    request_id: &[u8],
) -> Option<Result<Vec<u8>, Ended>> {
    let selected = match request {
        ClientRequest::ListAuthenticators => Ok(None),
        ClientRequest::GetAuthenticator { authenticator } => {
            AuthenticatorId::try_from(authenticator.as_slice())
                .map(Some)
                .map_err(|_| (ErrorCode::ErrorInvalidArgument, "invalid authenticator ID"))
        }
        _ => return None,
    };
    let Some(version) = connection.session.negotiated() else {
        return Some(Err(Ended::Refused("version unavailable".into())));
    };
    let payload = selected.and_then(|selected| {
        let inventory = dispatcher
            .inventory_for(connection)
            .map_err(|error| match error {
                AuthorityRefusal::Denied => {
                    (ErrorCode::ErrorUnauthorized, "inventory unauthorized")
                }
                _ => (ErrorCode::ErrorUnavailable, "inventory unavailable"),
            })?;
        match selected {
            None => Ok(Payload::Authenticators(inventory)),
            Some(id) => inventory
                .entries
                .into_iter()
                .find(|entry| entry.authenticator_id == id.as_bytes().as_slice())
                .map(|descriptor| {
                    Payload::Authenticator(Authenticator {
                        descriptor: Some(descriptor),
                    })
                })
                .ok_or((ErrorCode::ErrorUnavailable, "authenticator unavailable")),
        }
    });
    let mut frame = match payload {
        Ok(payload) => ClientFrame {
            request_id: request_id.to_vec(),
            payload: Some(payload),
            ..Default::default()
        },
        Err((code, message)) => error_frame(request_id, code, message),
    };
    frame.major = version.major;
    frame.minor = version.minor;
    Some(Ok(frame.encode_to_vec()))
}
