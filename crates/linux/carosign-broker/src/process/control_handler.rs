use std::time::{Duration, Instant};

use carosign_protocol::carosign::broker_relay::v1::{Control, ControlResult, control, control_result};
use prost::Message;

use crate::transport::{PacketSocket, TransportError};

use super::{
    owner_revocation::{await_owner_revoke, revoke_owner},
    packet_deadline::send_packet,
    request_validation::valid_id,
    resource_binding::ResourceBinding,
    session_state::SessionState,
};

pub(super) fn poll_control(
    socket: &PacketSocket,
    state: &mut SessionState,
    resource_binding: &mut ResourceBinding,
    pending_events: bool,
) -> Result<bool, TransportError> {
    match socket.receive() {
        Ok(raw) => {
            let request = Control::decode(raw.as_slice()).map_err(|_| TransportError::Invalid)?;
            if request.encode_to_vec() != raw
                || !valid_id(&request.request_id)
                || state.lease_revoked
            {
                return Err(TransportError::Invalid);
            }
            let Some(control::Action::Revoke(revoke)) = request.action else {
                return Err(TransportError::Invalid);
            };
            let request_id: [u8; 16] = request
                .request_id
                .as_slice()
                .try_into()
                .map_err(|_| TransportError::Invalid)?;
            let observation = revoke_owner(
                state.scope.as_ref(),
                revoke.scope.as_ref(),
                !state.replies.is_empty(),
                pending_events,
                || {
                    await_owner_revoke(
                        resource_binding.owner_session.as_mut().ok_or(())?,
                        request_id,
                    )
                },
            )
            .map_err(|_| TransportError::Invalid)?;
            state.lease_revoked = true;
            let response = ControlResult {
                request_id: request.request_id,
                result: Some(match observation {
                    carosign_protocol::resource_owner::Quiescence::ExclusiveQuiesced => {
                        control_result::Result::ExclusiveQuiesced(true)
                    }
                    carosign_protocol::resource_owner::Quiescence::ExclusiveLeaseRevoked => {
                        control_result::Result::ExclusiveRevoked(true)
                    }
                    _ => control_result::Result::Revoked(true),
                }),
            }
            .encode_to_vec();
            send_packet(socket, &response, Instant::now() + Duration::from_secs(5))?;
            Ok(true)
        }
        Err(TransportError::WouldBlock) => Ok(false),
        Err(error) => Err(error),
    }
}
