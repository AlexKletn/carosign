use carosign_linux_ipc::client::transport::TransportError;
use carosignctl::client::ClientError;

pub(super) fn map_error(error: TransportError) -> ClientError {
    match error {
        TransportError::Unavailable => ClientError::Unavailable,
        TransportError::Unexpected => ClientError::Unexpected,
    }
}
