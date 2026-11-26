use super::provider_channel::ProviderChannel;
use super::provider_failure::ProviderFailure;
use carosign_core::ids::OperationId;
use carosign_protocol::carosign::provider::v1 as provider;

pub(super) struct MutationFrameOnly;

impl ProviderChannel for MutationFrameOnly {
    fn send(&self, _: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        Err(ProviderFailure::Rejected)
    }
    fn cancel(&self, _: OperationId) -> bool {
        false
    }
}
