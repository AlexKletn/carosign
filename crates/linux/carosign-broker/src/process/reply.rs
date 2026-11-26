use carosign_protocol::carosign::broker_relay::v1::{
    BindResult, CommandResult, ControlResult, bind_result, command_result, control_result,
    frame::Body,
};

use crate::transport::TransportError;

#[derive(Clone, Copy)]
pub(super) enum Reply {
    Command,
    Bind,
    Cancel,
}

impl Reply {
    pub(super) fn into_body(
        self,
        request_id: [u8; 16],
        is_control: bool,
    ) -> Result<Body, TransportError> {
        Ok(match self {
            Reply::Command if !is_control => Body::CommandResult(CommandResult {
                request_id: request_id.to_vec(),
                result: Some(command_result::Result::Sent(true)),
            }),
            Reply::Bind if !is_control => Body::BindResult(BindResult {
                request_id: request_id.to_vec(),
                result: Some(bind_result::Result::Sent(true)),
            }),
            Reply::Cancel if is_control => Body::ControlResult(ControlResult {
                request_id: request_id.to_vec(),
                result: Some(control_result::Result::CancelSent(true)),
            }),
            _ => return Err(TransportError::Invalid),
        })
    }
}
