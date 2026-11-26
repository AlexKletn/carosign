use std::time::{Duration, Instant};

use carosign_protocol::carosign::broker_relay::v1::{Frame, Hello, frame::Body};
use prost::Message;

use crate::relay::HelloGate;
use crate::transport::{PacketSocket, TransportError};

use super::{
    Context,
    packet_deadline::{send_packet, wait_packet},
};

pub(super) fn exchange_hello(
    tunnel: &PacketSocket,
    context: &Context,
) -> Result<(), TransportError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut gate =
        HelloGate::new(context.worker, context.epoch).map_err(|_| TransportError::Invalid)?;
    let hello = wait_packet(tunnel, deadline)?;
    if gate
        .accept(&hello, 0)
        .map_err(|_| TransportError::Invalid)?
        != (context.broker, context.boot)
    {
        return Err(TransportError::Invalid);
    }
    let response = Frame {
        major: 1,
        minor: 0,
        seq: 1,
        worker_instance: context.worker.to_vec(),
        device_epoch: context.epoch,
        body: Some(Body::Hello(Hello {
            broker_instance: context.broker.to_vec(),
            boot_id: context.boot.to_vec(),
        })),
    }
    .encode_to_vec();
    send_packet(tunnel, &response, deadline)
}
