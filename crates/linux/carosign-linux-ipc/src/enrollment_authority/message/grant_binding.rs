use carosign_core::authorization::ValidatedGrant;
use prost::Message;

use super::valid_id;

/// The context of a trusted authority snapshot, on the service side, should be independently
/// verified against a live source, not accepted from the request as a right.
#[derive(Clone, PartialEq, Message)]
pub struct GrantBinding {
    #[prost(bytes = "vec", tag = "1")]
    pub id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub issuer: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub deadline_ns: u64,
}

impl GrantBinding {
    pub fn from_grant(grant: &ValidatedGrant) -> Self {
        Self {
            id: grant.id().as_bytes().to_vec(),
            issuer: grant.issuer().as_bytes().to_vec(),
            deadline_ns: grant.deadline_ns(),
        }
    }

    pub(in crate::enrollment_authority) fn valid(&self) -> bool {
        valid_id(&self.id) && valid_id(&self.issuer) && self.deadline_ns != 0
    }
}
