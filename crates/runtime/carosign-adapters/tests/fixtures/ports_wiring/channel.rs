use carosign_adapters::provider::{ProviderChannel, ProviderFailure};
use carosign_core::ids::OperationId;
use carosign_protocol::carosign::provider::v1 as provider;

pub(super) struct Channel;

impl ProviderChannel for Channel {
    fn send(&self, _frame: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        Ok(())
    }
    fn cancel(&self, _operation: OperationId) -> bool {
        true
    }
}
