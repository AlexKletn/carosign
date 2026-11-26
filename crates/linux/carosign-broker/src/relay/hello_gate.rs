use super::Refusal;

use carosign_protocol::carosign::broker_relay::v1::{Frame, frame::Body};
use prost::Message;
use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};

pub(super) fn id(bytes: &[u8]) -> bool {
    bytes.len() == 16 && bytes.iter().any(|byte| *byte != 0)
}

// prost stores the last singular/oneof value and skips unknown fields, so check the original
// Hello form before decoding.
fn strict_hello(mut bytes: &[u8], nested: bool) -> Result<(), Refusal> {
    let mut seen = 0u32;
    while !bytes.is_empty() {
        let (tag, wire) = decode_key(&mut bytes).map_err(|_| Refusal::Invalid)?;
        let expected = match (nested, tag) {
            (false, 1 | 2 | 3 | 5) => WireType::Varint,
            (false, 4 | 10) | (true, 1 | 2) => WireType::LengthDelimited,
            _ => return Err(Refusal::Invalid),
        };
        if wire != expected || seen & (1u32 << tag) != 0 {
            return Err(Refusal::Invalid);
        }
        seen |= 1u32 << tag;
        if !nested && tag == 10 {
            let length = usize::try_from(decode_varint(&mut bytes).map_err(|_| Refusal::Invalid)?)
                .map_err(|_| Refusal::Invalid)?;
            if length > bytes.len() {
                return Err(Refusal::Invalid);
            }
            let (body, rest) = bytes.split_at(length);
            strict_hello(body, true)?;
            bytes = rest;
        } else {
            skip_field(wire, tag, &mut bytes, DecodeContext::default())
                .map_err(|_| Refusal::Invalid)?;
        }
    }
    Ok(())
}

/// Accepts exactly one initial frame; no further wire has yet been connected.
#[allow(dead_code)]
pub(crate) struct HelloGate {
    worker: [u8; 16],
    epoch: u64,
    closed: bool,
}

#[allow(dead_code)]
impl HelloGate {
    pub(crate) fn new(worker: [u8; 16], epoch: u64) -> Result<Self, Refusal> {
        if !id(&worker) || epoch == 0 {
            return Err(Refusal::Invalid);
        }
        Ok(Self {
            worker,
            epoch,
            closed: false,
        })
    }

    pub(crate) fn accept(
        &mut self,
        bytes: &[u8],
        descriptors: usize,
    ) -> Result<([u8; 16], [u8; 16]), Refusal> {
        if self.closed {
            return Err(Refusal::Closed);
        }
        self.closed = true;
        if descriptors != 0 || bytes.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(Refusal::Invalid);
        }
        strict_hello(bytes, false)?;
        let frame = Frame::decode(bytes).map_err(|_| Refusal::Invalid)?;
        let Some(Body::Hello(hello)) = frame.body else {
            return Err(Refusal::Invalid);
        };
        if frame.major != 1
            || frame.minor != 0
            || frame.seq != 1
            || frame.worker_instance != self.worker
            || frame.device_epoch != self.epoch
            || !id(&hello.broker_instance)
            || !id(&hello.boot_id)
        {
            return Err(Refusal::Invalid);
        }
        Ok((
            hello
                .broker_instance
                .try_into()
                .map_err(|_| Refusal::Invalid)?,
            hello.boot_id.try_into().map_err(|_| Refusal::Invalid)?,
        ))
    }
}

/// The query ID is not a right; the conversion is unique in an instance.
#[allow(dead_code)]
pub(crate) fn request_id(instance: [u8; 16], seq: u64) -> Result<[u8; 16], Refusal> {
    if !id(&instance) || seq == 0 {
        return Err(Refusal::Invalid);
    }
    let mut result = instance;
    for (destination, source) in result[8..].iter_mut().zip(seq.to_be_bytes()) {
        *destination ^= source;
    }
    if !id(&result) {
        return Err(Refusal::Invalid);
    }
    Ok(result)
}

#[path = "hello_gate.tests.rs"]
#[cfg(test)]
mod tests;
