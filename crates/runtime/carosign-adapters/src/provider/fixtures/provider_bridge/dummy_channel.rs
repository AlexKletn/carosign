use carosign_core::ids::OperationId;
use carosign_protocol::carosign::provider::v1 as provider;

use crate::provider::{ProviderChannel, ProviderFailure};

pub(super) struct Dummy;

impl ProviderChannel for Dummy {
    fn send(&self, _frame: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        Ok(())
    }
    fn cancel(&self, _operation: OperationId) -> bool {
        true
    }
}
