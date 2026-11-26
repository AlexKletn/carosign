use carosign_linux_ipc::client::transport::Transport;
use carosign_protocol::carosign::client::v1 as wire;
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Cancellation {
    pub(super) cancelled: bool,
    pub(super) operation: Option<(Arc<Transport>, Vec<u8>)>,
}

pub(super) fn request_cancel(transport: Arc<Transport>, operation_id: Vec<u8>) {
    std::thread::spawn(move || {
        let _ = transport.request(wire::client_frame::Payload::CancelOperation(
            wire::CancelOperation { operation_id },
        ));
    });
}
