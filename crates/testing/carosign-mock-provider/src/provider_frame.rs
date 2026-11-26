use carosign_protocol::carosign::provider::v1::{self as provider, provider_frame::Payload};

pub(crate) fn frame(
    instance: Vec<u8>,
    epoch: u64,
    command: Vec<u8>,
    operation: Vec<u8>,
    payload: Payload,
) -> provider::ProviderFrame {
    provider::ProviderFrame {
        major: 1,
        minor: carosign_protocol::limits::MINOR_BASELINE,
        worker_instance_id: instance,
        frame_seq: 0,
        command_id: command,
        operation_id: operation,
        device_epoch: epoch,
        uid: None,
        payload: Some(payload),
    }
}
