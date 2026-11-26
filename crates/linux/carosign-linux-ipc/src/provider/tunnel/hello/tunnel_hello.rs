use super::HelloStage;

use super::super::command_encoder::valid_id;
use carosign_protocol::carosign::broker_relay::v1::{Frame, Hello, frame::Body};
use prost::encoding::{DecodeContext, WireType, decode_key, decode_varint, skip_field};

use super::super::HelloError;
use prost::Message;

fn strict_hello(mut bytes: &[u8], nested: bool) -> Result<(), HelloError> {
    let mut seen = 0u32;
    while !bytes.is_empty() {
        let (tag, wire) = decode_key(&mut bytes).map_err(|_| HelloError::Invalid)?;
        let expected = match (nested, tag) {
            (false, 1 | 2 | 3 | 5) => WireType::Varint,
            (false, 4 | 10) | (true, 1 | 2) => WireType::LengthDelimited,
            _ => return Err(HelloError::Invalid),
        };
        if wire != expected || seen & (1u32 << tag) != 0 {
            return Err(HelloError::Invalid);
        }
        seen |= 1u32 << tag;
        if !nested && tag == 10 {
            let len = usize::try_from(decode_varint(&mut bytes).map_err(|_| HelloError::Invalid)?)
                .map_err(|_| HelloError::Invalid)?;
            if len > bytes.len() {
                return Err(HelloError::Invalid);
            }
            let (body, rest) = bytes.split_at(len);
            strict_hello(body, true)?;
            bytes = rest;
        } else {
            skip_field(wire, tag, &mut bytes, DecodeContext::default())
                .map_err(|_| HelloError::Invalid)?;
        }
    }
    Ok(())
}

/// Expected identifiers are set by the composition, not the response of the broker.
pub struct TunnelHello {
    worker: [u8; 16],
    epoch: u64,
    broker: [u8; 16],
    boot: [u8; 16],
    stage: HelloStage,
}

impl TunnelHello {
    /// Creates an initial state; all identifiers must be non-zero.
    pub fn new(
        worker: [u8; 16],
        epoch: u64,
        broker: [u8; 16],
        boot: [u8; 16],
    ) -> Result<Self, HelloError> {
        if !valid_id(&worker) || epoch == 0 || !valid_id(&broker) || !valid_id(&boot) {
            return Err(HelloError::Invalid);
        }
        Ok(Self {
            worker,
            epoch,
            broker,
            boot,
            stage: HelloStage::Prepared,
        })
    }

    /// Forms the first daemon → broker message; sending it does not mean trust.
    pub fn outgoing(&mut self) -> Result<Vec<u8>, HelloError> {
        if self.stage != HelloStage::Prepared {
            return Err(HelloError::Closed);
        }
        self.stage = HelloStage::Offered;
        Ok(Frame {
            major: 1,
            minor: 0,
            seq: 1,
            worker_instance: self.worker.to_vec(),
            device_epoch: self.epoch,
            body: Some(Body::Hello(Hello {
                broker_instance: self.broker.to_vec(),
                boot_id: self.boot.to_vec(),
            })),
        }
        .encode_to_vec())
    }

    /// Accepts exactly one answer. Any incorrect packet completes the check.
    pub fn accept(&mut self, bytes: &[u8], descriptors: usize) -> Result<(), HelloError> {
        if self.stage != HelloStage::Offered {
            self.stage = HelloStage::Closed;
            return Err(HelloError::Closed);
        }
        self.stage = HelloStage::Closed;
        if descriptors != 0 || bytes.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(HelloError::Invalid);
        }
        strict_hello(bytes, false)?;
        let frame = Frame::decode(bytes).map_err(|_| HelloError::Invalid)?;
        match frame.body {
            Some(Body::Hello(hello))
                if frame.major == 1
                    && frame.minor == 0
                    && frame.seq == 1
                    && frame.worker_instance == self.worker
                    && frame.device_epoch == self.epoch
                    && hello.broker_instance == self.broker
                    && hello.boot_id == self.boot =>
            {
                Ok(())
            }
            _ => Err(HelloError::Invalid),
        }
    }
}
