use std::os::fd::OwnedFd;
use std::time::Duration;

use carosign_protocol::carosign::broker_relay::v1::{ControlResult, control_result, frame::Body};

use crate::relay_pump::{ReceiveRequest, RelayPump};
use crate::transport::{PacketSocket, TransportError};

use super::{
    ConfiguredResource, Context, control_handler::poll_control, handshake::exchange_hello,
    provider_event::check_provider_event, request_handler::check_request,
    resource_binding::ResourceBinding, session_state::SessionState,
};

/// Runs a limited process on two transferred supervisor connections.
pub fn run(tunnel: OwnedFd, provider: OwnedFd, context: Context) -> Result<(), TransportError> {
    run_with_resource(tunnel, provider, context, None)
}

/// Additionally, one system-configured FD is allowed within the issuance.
pub fn run_with_resource(
    tunnel: OwnedFd,
    provider: OwnedFd,
    context: Context,
    resource: Option<ConfiguredResource>,
) -> Result<(), TransportError> {
    run_with_resources(tunnel, provider, context, resource.into_iter().collect())
}

/// Transmits the full set-up only after checking the entire binding.
pub fn run_with_resources(
    tunnel: OwnedFd,
    provider: OwnedFd,
    context: Context,
    resources: Vec<ConfiguredResource>,
) -> Result<(), TransportError> {
    run_with_resource_owner(tunnel, provider, context, resources, None)
}

/// A trusted supervisor with a private owner channel.
/// The caller must secure the scope until Bind is sent to the tunnel; the mere existence of
/// the channel does not confirm the right or origin of the owner.
pub fn run_with_resource_owner(
    tunnel: OwnedFd,
    provider: OwnedFd,
    context: Context,
    resources: Vec<ConfiguredResource>,
    owner_channel: Option<OwnedFd>,
) -> Result<(), TransportError> {
    run_with_control(
        tunnel,
        provider,
        context,
        resources,
        owner_channel,
        None,
        None,
    )
}

/// A separate channel is issued by the supervisor, not the provider’s worker.
pub fn run_with_control(
    tunnel: OwnedFd,
    provider: OwnedFd,
    context: Context,
    resources: Vec<ConfiguredResource>,
    owner_channel: Option<OwnedFd>,
    control_channel: Option<OwnedFd>,
    owner_managed: Option<[u8; 16]>,
) -> Result<(), TransportError> {
    run_with_provider(
        tunnel,
        PacketSocket::new(provider),
        context,
        resources,
        owner_channel,
        control_channel,
        owner_managed,
    )
}

/// Receives a channel of the worker verified by the platform after OpenSession.
pub fn run_with_discovered_worker(
    tunnel: OwnedFd,
    provider: OwnedFd,
    context: Context,
    resources: Vec<ConfiguredResource>,
    owner_channel: Option<OwnedFd>,
    control_channel: Option<OwnedFd>,
    owner_managed: Option<[u8; 16]>,
) -> Result<(), TransportError> {
    run_with_provider(
        tunnel,
        PacketSocket::from_discovered_worker(provider),
        context,
        resources,
        owner_channel,
        control_channel,
        owner_managed,
    )
}

fn run_with_provider(
    tunnel: OwnedFd,
    provider: Result<PacketSocket, TransportError>,
    context: Context,
    resources: Vec<ConfiguredResource>,
    owner_channel: Option<OwnedFd>,
    control_channel: Option<OwnedFd>,
    owner_managed: Option<[u8; 16]>,
) -> Result<(), TransportError> {
    let mut resource_binding = ResourceBinding::new(resources, owner_channel, owner_managed)?;
    // The successful owner stays alive until the provider channel closes.
    let control_channel = control_channel.map(PacketSocket::new).transpose()?;
    let tunnel = PacketSocket::new(tunnel)?;
    let provider = provider?;
    exchange_hello(&tunnel, &context)?;
    let mut pump = RelayPump::new(provider, tunnel);
    let mut state = SessionState::new(context);
    loop {
        let pending_events = pump.has_pending_events();
        let mut progress = false;
        if let Some(control_socket) = &control_channel {
            progress = poll_control(
                control_socket,
                &mut state,
                &mut resource_binding,
                pending_events,
            )?;
        }
        match pump.receive_request(|raw| {
            check_request(&mut state, &mut resource_binding, pending_events, raw)
        }) {
            Ok(ReceiveRequest::Local { request_id }) => {
                queue_reply(
                    &mut pump,
                    &mut state,
                    Body::ControlResult(ControlResult {
                        request_id: request_id.to_vec(),
                        result: Some(control_result::Result::Revoked(true)),
                    }),
                )?;
                progress = true;
            }
            Ok(ReceiveRequest::Queued { .. }) => progress = true,
            Ok(ReceiveRequest::Busy { .. }) => return Err(TransportError::Invalid),
            Err(TransportError::WouldBlock) => {}
            Err(error) => return Err(error),
        }
        match pump.send_sequenced_command(true, &mut state.provider_seq) {
            Ok(true) => {
                let receipt = pump.take_sent_receipt().ok_or(TransportError::Invalid)?;
                let reply = state
                    .replies
                    .remove(&receipt.request_id)
                    .ok_or(TransportError::Invalid)?;
                let body = reply.into_body(receipt.request_id, receipt.is_control)?;
                queue_reply(&mut pump, &mut state, body)?;
                progress = true;
            }
            Ok(false) | Err(TransportError::WouldBlock) => {}
            Err(error) => return Err(error),
        }
        match pump.receive_event(|raw| check_provider_event(&mut state, raw)) {
            Ok(true) => progress = true,
            Ok(false) | Err(TransportError::WouldBlock) => {}
            Err(error) => return Err(error),
        }
        match pump.send_event() {
            Ok(true) => progress = true,
            Ok(false) | Err(TransportError::WouldBlock) => {}
            Err(error) => return Err(error),
        }
        if !progress {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn queue_reply(
    pump: &mut RelayPump,
    state: &mut SessionState,
    body: Body,
) -> Result<(), TransportError> {
    pump.event(state.outbound_frame(body))
        .map_err(|_| TransportError::Invalid)?;
    state
        .advance_outbound()
        .map_err(|_| TransportError::Invalid)
}
