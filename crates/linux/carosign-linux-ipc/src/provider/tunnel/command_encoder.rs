use std::collections::HashSet;

use carosign_protocol::carosign::broker_relay::v1::{
    Bind, Cancel, Command, Control, Frame, MutationPermit, Revoke, Scope, Slot, control,
    frame::Body,
};
use carosign_protocol::carosign::provider::v1::{ProviderFrame, provider_frame::Payload};

use super::HelloError;
use prost::Message;

/// Create a single request after a successful Hello exchange.
/// The condition does not certify the socket, authority or delivery of the command.
pub struct TunnelCommandEncoder {
    worker: [u8; 16],
    epoch: u64,
    broker: [u8; 16],
    next: u64,
    closed: bool,
    cancel_ids: HashSet<[u8; 16]>,
}

impl TunnelCommandEncoder {
    /// It is created by the owner of the connection after checking Hello.
    pub fn after_hello(worker: [u8; 16], epoch: u64, broker: [u8; 16]) -> Result<Self, HelloError> {
        if !valid_id(&worker) || epoch == 0 || !valid_id(&broker) {
            return Err(HelloError::Invalid);
        }
        Ok(Self {
            worker,
            epoch,
            broker,
            next: 2,
            closed: false,
            cancel_ids: HashSet::new(),
        })
    }

    /// Forms limited management of the output. The broker independently confirms the
    /// scope; the frame construction is not a recall or cancellation.
    pub fn control(
        &mut self,
        scope: Scope,
        cancel_id: Option<[u8; 16]>,
    ) -> Result<(Vec<u8>, [u8; 16]), HelloError> {
        let result = self.encode_control(scope, cancel_id);
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    fn encode_control(
        &mut self,
        scope: Scope,
        cancel_id: Option<[u8; 16]>,
    ) -> Result<(Vec<u8>, [u8; 16]), HelloError> {
        if self.closed {
            return Err(HelloError::Closed);
        }
        let owner = scope.owner.as_ref().ok_or(HelloError::Invalid)?;
        if !valid_id(&scope.lease_id)
            || !valid_id(&owner.operation_id)
            || !valid_id(&owner.authenticator_id)
            || owner.worker_instance != self.worker
            || owner.device_epoch != self.epoch
            || scope.claims.is_empty()
            || scope.claims.len() > 8
        {
            return Err(HelloError::Invalid);
        }
        let mut previous: Option<&[u8]> = None;
        for claim in &scope.claims {
            if !valid_id(&claim.resource_id)
                || !(1..=3).contains(&claim.mode)
                || previous.is_some_and(|p| p >= claim.resource_id.as_slice())
            {
                return Err(HelloError::Invalid);
            }
            previous = Some(&claim.resource_id);
        }
        if let Some(id) = cancel_id
            && (!valid_id(&id) || self.cancel_ids.len() >= 1024 || self.cancel_ids.contains(&id))
        {
            return Err(HelloError::Invalid);
        }
        let seq = self.next;
        let next = seq.checked_add(1).ok_or(HelloError::Closed)?;
        let mut request = self.broker;
        for (dst, value) in request[8..].iter_mut().zip(seq.to_be_bytes()) {
            *dst ^= value;
        }
        if !valid_id(&request) {
            return Err(HelloError::Invalid);
        }
        let action = match cancel_id {
            Some(id) => control::Action::Cancel(Cancel {
                scope: Some(scope),
                cancel_command_id: id.to_vec(),
            }),
            None => control::Action::Revoke(Revoke { scope: Some(scope) }),
        };
        let bytes = Frame {
            major: 1,
            minor: 0,
            seq,
            worker_instance: self.worker.to_vec(),
            device_epoch: self.epoch,
            body: Some(Body::Control(Control {
                request_id: request.to_vec(),
                action: Some(action),
            })),
        }
        .encode_to_vec();
        if bytes.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(HelloError::Invalid);
        }
        if let Some(id) = cancel_id {
            self.cancel_ids.insert(id);
        }
        self.next = next;
        Ok((bytes, request))
    }

    /// Builds only the permitted regular command; Bind and Cancel go their separate way.
    /// After the return, the caller is obliged to close the channel if the outcome of the
    /// shipment is unknown.
    pub fn command(&mut self, frame: ProviderFrame) -> Result<(Vec<u8>, [u8; 16]), HelloError> {
        self.encode_command(frame, None)
    }

    pub(super) fn encode_command(
        &mut self,
        frame: ProviderFrame,
        permit: Option<MutationPermit>,
    ) -> Result<(Vec<u8>, [u8; 16]), HelloError> {
        if self.closed {
            return Err(HelloError::Closed);
        }
        let valid_payload = matches!(
            frame.payload,
            Some(
                Payload::Initialize(_)
                    | Payload::Describe(_)
                    | Payload::PrebindDescribe(_)
                    | Payload::StartVerification(_)
                    | Payload::PrepareEnrollmentChallenge(_)
                    | Payload::StartEnrollment(_)
                    | Payload::ContinueEnrollment(_)
                    | Payload::StartRemoval(_)
                    | Payload::QueryMutation(_)
                    | Payload::EnrollmentTransactionRequest(_)
                    | Payload::Quiesce(_)
                    | Payload::CloseAuthenticator(_)
                    | Payload::Shutdown(_)
            )
        );
        let is_initialize = matches!(frame.payload, Some(Payload::Initialize(_)));
        let is_mutation = matches!(
            frame.payload,
            Some(
                Payload::StartEnrollment(_)
                    | Payload::ContinueEnrollment(_)
                    | Payload::StartRemoval(_)
            )
        );
        if !valid_payload
            || is_mutation != permit.is_some()
            || frame.frame_seq != 0
            || frame.worker_instance_id != self.worker
            || frame.device_epoch != if is_initialize { 0 } else { self.epoch }
        {
            self.closed = true;
            return Err(HelloError::Invalid);
        }
        let seq = self.next;
        let Some(next) = seq.checked_add(1) else {
            self.closed = true;
            return Err(HelloError::Closed);
        };
        let mut request = self.broker;
        for (dst, value) in request[8..].iter_mut().zip(seq.to_be_bytes()) {
            *dst ^= value;
        }
        if !valid_id(&request) {
            self.closed = true;
            return Err(HelloError::Invalid);
        }
        let bytes = Frame {
            major: 1,
            minor: 0,
            seq,
            worker_instance: self.worker.to_vec(),
            device_epoch: self.epoch,
            body: Some(Body::Command(Command {
                request_id: request.to_vec(),
                frame: Some(frame),
                mutation_permit: permit,
            })),
        }
        .encode_to_vec();
        if bytes.len() > carosign_protocol::limits::FRAME_MAX {
            self.closed = true;
            return Err(HelloError::Invalid);
        }
        self.next = next;
        Ok((bytes, request))
    }

    /// Only generates a Bind query without descriptors. Authorities and resource openings are
    /// independently verified by the broker; this frame is not confirmed by Bind.
    pub fn bind(
        &mut self,
        scope: Scope,
        slots: Vec<Slot>,
        frame: ProviderFrame,
    ) -> Result<(Vec<u8>, [u8; 16]), HelloError> {
        let result = self.encode_bind(scope, slots, frame);
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    fn encode_bind(
        &mut self,
        scope: Scope,
        slots: Vec<Slot>,
        frame: ProviderFrame,
    ) -> Result<(Vec<u8>, [u8; 16]), HelloError> {
        if self.closed {
            return Err(HelloError::Closed);
        }
        let owner = scope.owner.as_ref().ok_or(HelloError::Invalid)?;
        let Some(Payload::BindAuthenticator(bind)) = frame.payload.as_ref() else {
            return Err(HelloError::Invalid);
        };
        let authenticator = bind.authenticator.as_ref().ok_or(HelloError::Invalid)?;
        if !valid_id(&scope.lease_id)
            || !valid_id(&owner.operation_id)
            || !valid_id(&owner.authenticator_id)
            || owner.worker_instance != self.worker
            || owner.device_epoch != self.epoch
            || frame.worker_instance_id != self.worker
            || frame.device_epoch != self.epoch
            || frame.frame_seq != 0
            || authenticator.authenticator_id != owner.authenticator_id
            || authenticator.device_epoch != self.epoch
            || bind.lease_id != scope.lease_id
            || !valid_id(&bind.profile_id)
            || scope.claims.is_empty()
            || scope.claims.len() > 8
            || slots.len() != bind.fd_slots.len()
            || slots.len() > 8
        {
            return Err(HelloError::Invalid);
        }
        let mut previous: Option<&[u8]> = None;
        for claim in &scope.claims {
            if !valid_id(&claim.resource_id)
                || !(1..=3).contains(&claim.mode)
                || previous.is_some_and(|p| p >= claim.resource_id.as_slice())
            {
                return Err(HelloError::Invalid);
            }
            previous = Some(&claim.resource_id);
        }
        for (index, (slot, fd)) in slots.iter().zip(&bind.fd_slots).enumerate() {
            if !valid_id(&slot.resource_id)
                || !(1..=4).contains(&slot.role)
                || slot.role != fd.role
                || slot.resource_id != fd.resource_id
                || fd.index as usize != index
                || !scope
                    .claims
                    .iter()
                    .any(|c| c.resource_id == slot.resource_id)
                || slots[..index]
                    .iter()
                    .any(|s| s.resource_id == slot.resource_id)
            {
                return Err(HelloError::Invalid);
            }
        }
        let seq = self.next;
        let next = seq.checked_add(1).ok_or(HelloError::Closed)?;
        let mut request = self.broker;
        for (dst, value) in request[8..].iter_mut().zip(seq.to_be_bytes()) {
            *dst ^= value;
        }
        if !valid_id(&request) {
            return Err(HelloError::Invalid);
        }
        let bytes = Frame {
            major: 1,
            minor: 0,
            seq,
            worker_instance: self.worker.to_vec(),
            device_epoch: self.epoch,
            body: Some(Body::Bind(Bind {
                request_id: request.to_vec(),
                scope: Some(scope),
                slots,
                frame: Some(frame),
            })),
        }
        .encode_to_vec();
        if bytes.len() > carosign_protocol::limits::FRAME_MAX {
            return Err(HelloError::Invalid);
        }
        self.next = next;
        Ok((bytes, request))
    }
}

pub(super) fn valid_id(value: &[u8]) -> bool {
    value.len() == 16 && value.iter().any(|v| *v != 0)
}

// Prost skips unknown fields and replaces repeated singular fields.
// Check the original bytes before decoding, including the nested Hello.
