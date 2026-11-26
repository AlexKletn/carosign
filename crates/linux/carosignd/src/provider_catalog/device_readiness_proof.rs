use carosign_protocol::carosign::common::v1::{CapabilityDescriptor, UnavailabilityReason};

use super::{RemoteProvider, endpoint_identity::EndpointIdentity};

pub(super) struct DeviceReadinessProof {
    pub(super) remote: RemoteProvider,
    pub(super) result: Result<CapabilityDescriptor, UnavailabilityReason>,
}

impl DeviceReadinessProof {
    pub(super) fn matches(&self, binding: Option<(EndpointIdentity, u64)>) -> bool {
        binding == Some((self.remote.identity, self.remote.epoch().value()))
            && self.remote.profile_is_current()
    }
}

#[cfg(test)]
#[path = "device_readiness_proof.tests.rs"]
mod tests;
