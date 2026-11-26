use carosign_core::{
    device::DeviceEpoch,
    ids::{AuthenticatorId, ProfileId, ProviderId},
};

use crate::limits;

/// The authenticator record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AuthenticatorRecord {
    pub authenticator: AuthenticatorId,
    /// The ISP family.
    pub provider: ProviderId,
    pub profile: ProfileId,
    /// Hash hardware identity.
    pub hardware_digest: [u8; limits::DIGEST_LEN],
    pub epoch: DeviceEpoch,
    pub available: bool,
}
