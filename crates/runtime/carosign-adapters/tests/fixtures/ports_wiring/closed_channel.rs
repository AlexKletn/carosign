use carosign_adapters::provider::{ProviderChannel, ProviderFailure};
use carosign_core::ids::OperationId;
use carosign_protocol::carosign::provider::v1 as provider;

pub(super) struct ClosedChannel;

impl ProviderChannel for ClosedChannel {
    fn send(&self, _frame: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        Err(ProviderFailure::SendFailed)
    }
    fn cancel(&self, _operation: OperationId) -> bool {
        false
    }
}
