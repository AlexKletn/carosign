use super::Refusal;
use super::hello_gate::id;
use super::request_id;
use carosign_protocol::carosign::broker_relay::v1::Frame;
use carosign_protocol::carosign::broker_relay::v1::frame::Body;
use prost::{
    Message,
    encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field},
};

/// Limited analysis of the Shutdown command by source bytes. Permissible commands are
/// intentionally not expanded without checking all nested fields. Wire and context check is
/// not permission to send.
#[allow(dead_code)]
pub(crate) struct ShutdownGate {
    worker: [u8; 16],
    epoch: u64,
    instance: [u8; 16],
    next_seq: u64,
    closed: bool,
}

#[allow(dead_code)]
impl ShutdownGate {
    pub(crate) fn new(worker: [u8; 16], epoch: u64, instance: [u8; 16]) -> Result<Self, Refusal> {
        if !id(&worker) || epoch == 0 || !id(&instance) {
            return Err(Refusal::Invalid);
        }
        Ok(Self {
            worker,
            epoch,
            instance,
            next_seq: 2,
            closed: false,
        })
    }

    pub(crate) fn accept(
        &mut self,
        bytes: &[u8],
    ) -> Result<
        (
            [u8; 16],
            carosign_protocol::carosign::provider::v1::ProviderFrame,
        ),
        Refusal,
    > {
        if self.closed {
            return Err(Refusal::Closed);
        }
        let result = self.validate(bytes);
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    fn validate(
        &mut self,
        bytes: &[u8],
    ) -> Result<
        (
            [u8; 16],
            carosign_protocol::carosign::provider::v1::ProviderFrame,
        ),
        Refusal,
    > {
        use carosign_protocol::carosign::provider::v1::provider_frame::Payload;
        if bytes.is_empty() || bytes.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(Refusal::Invalid);
        }
        strict_shutdown(bytes, 0)?;
        let frame = Frame::decode(bytes).map_err(|_| Refusal::Invalid)?;
        if frame.major != 1
            || frame.minor != 0
            || frame.seq != self.next_seq
            || frame.worker_instance != self.worker
            || frame.device_epoch != self.epoch
        {
            return Err(Refusal::Invalid);
        }
        let Some(Body::Command(command)) = frame.body else {
            return Err(Refusal::Invalid);
        };
        let expected = request_id(self.instance, frame.seq)?;
        if command.request_id != expected {
            return Err(Refusal::Invalid);
        }
        let provider = command.frame.ok_or(Refusal::Invalid)?;
        if provider.major != 1
            || provider.minor != carosign_protocol::limits::MINOR_BASELINE
            || provider.worker_instance_id != self.worker
            || provider.device_epoch != self.epoch
            || provider.frame_seq != 0
            || !matches!(provider.payload, Some(Payload::Shutdown(_)))
            || !id(&provider.command_id)
            || !provider.operation_id.is_empty()
        {
            return Err(Refusal::Invalid);
        }
        self.next_seq = self.next_seq.checked_add(1).ok_or(Refusal::Invalid)?;
        Ok((expected, provider))
    }
}

fn strict_shutdown(mut bytes: &[u8], level: u8) -> Result<(), Refusal> {
    let mut seen = 0u64;
    while !bytes.is_empty() {
        let (tag, wire) = decode_key(&mut bytes).map_err(|_| Refusal::Invalid)?;
        let expected = match (level, tag) {
            (0, 1 | 2 | 3 | 5) | (2, 1 | 2 | 4 | 7) => WireType::Varint,
            (0, 4 | 11) | (1, 1 | 2) | (2, 3 | 5 | 6 | 43) => WireType::LengthDelimited,
            _ => return Err(Refusal::Invalid),
        };
        if wire != expected || seen & (1u64 << tag) != 0 {
            return Err(Refusal::Invalid);
        }
        seen |= 1u64 << tag;
        if matches!((level, tag), (0, 11) | (1, 2) | (2, 43)) {
            let len = usize::try_from(decode_varint(&mut bytes).map_err(|_| Refusal::Invalid)?)
                .map_err(|_| Refusal::Invalid)?;
            if len > bytes.len() {
                return Err(Refusal::Invalid);
            }
            let (inner, rest) = bytes.split_at(len);
            if level == 2 {
                if !inner.is_empty() {
                    return Err(Refusal::Invalid);
                }
            } else {
                strict_shutdown(inner, level + 1)?;
            }
            bytes = rest;
        } else {
            skip_field(wire, tag, &mut bytes, DecodeContext::default())
                .map_err(|_| Refusal::Invalid)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "shutdown_gate.tests.rs"]
mod tests;
