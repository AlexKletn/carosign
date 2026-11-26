use std::collections::HashMap;

use carosign_protocol::carosign::broker_relay::v1::{Frame, Scope, frame::Body};
use prost::Message;

use super::{
    Context, active_operation::ActiveOperation, challenge_preparation::ChallengePreparation,
    provider_setup::ProviderSetup, reply::Reply,
};

pub(super) struct SessionState {
    pub(super) context: Context,
    pub(super) inbound: carosign_provider_sdk::wire::Channel,
    pub(super) provider_seq: u64,
    pub(super) inbound_seq: u64,
    outbound_seq: u64,
    pub(super) scope: Option<Scope>,
    pub(super) setup: ProviderSetup,
    pub(super) active_operation: Option<ActiveOperation>,
    pub(super) challenge_preparation: Option<ChallengePreparation>,
    pub(super) replies: HashMap<[u8; 16], Reply>,
    pub(super) lease_revoked: bool,
}

impl SessionState {
    pub(super) fn new(context: Context) -> Self {
        Self {
            context,
            inbound: carosign_provider_sdk::wire::Channel::new(),
            provider_seq: 1,
            inbound_seq: 2,
            outbound_seq: 2,
            scope: None,
            setup: ProviderSetup::default(),
            active_operation: None,
            challenge_preparation: None,
            replies: HashMap::new(),
            lease_revoked: false,
        }
    }

    pub(super) fn outbound_frame(&self, body: Body) -> Vec<u8> {
        Frame {
            major: 1,
            minor: 0,
            seq: self.outbound_seq,
            worker_instance: self.context.worker.to_vec(),
            device_epoch: self.context.epoch,
            body: Some(body),
        }
        .encode_to_vec()
    }

    pub(super) fn advance_outbound(&mut self) -> Result<(), ()> {
        self.outbound_seq = self.outbound_seq.checked_add(1).ok_or(())?;
        Ok(())
    }
}
