use carosign_core::ids::OperationId;
use carosign_linux_ipc::codec::error_frame;
use carosign_protocol::carosign::{
    client::v1::{CancellationReceipt, ClientFrame, client_frame::Payload},
    common::v1::ErrorCode,
};
use carosign_runtime::ports::AuthorityRefusal;
use prost::Message;

use super::{Connection, Ended, VerificationDispatcher};

pub(super) fn answer(
    connection: &Connection,
    dispatcher: &dyn VerificationDispatcher,
    operation: &[u8],
    cancel: bool,
    request_id: &[u8],
) -> Result<Vec<u8>, Ended> {
    let version = connection
        .session
        .negotiated()
        .ok_or_else(|| Ended::Refused("version unavailable".into()))?;
    let payload = OperationId::try_from(operation)
        .map_err(|_| (ErrorCode::ErrorInvalidArgument, "invalid operation ID"))
        .and_then(|operation| {
            let result = if cancel {
                dispatcher
                    .cancel_operation(connection, operation)
                    .map(|receipt| {
                        Payload::CancellationReceipt(CancellationReceipt {
                            result: receipt as i32,
                        })
                    })
            } else {
                dispatcher
                    .operation_status(connection, operation)
                    .map(Payload::OperationSnapshot)
            };
            result.map_err(|refusal| match refusal {
                AuthorityRefusal::Denied => {
                    (ErrorCode::ErrorUnauthorized, "operation unauthorized")
                }
                _ => (ErrorCode::ErrorUnavailable, "operation unavailable"),
            })
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
    Ok(frame.encode_to_vec())
}
