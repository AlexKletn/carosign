use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use carosign_core::ids::OperationId;
use carosign_protocol::carosign::provider::v1 as provider;

use crate::provider::{ProviderChannel, ProviderFailure};

pub(super) struct CountingChannel(pub(super) Arc<AtomicUsize>);

impl ProviderChannel for CountingChannel {
    fn send(&self, _: provider::ProviderFrame) -> Result<(), ProviderFailure> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn cancel(&self, _: OperationId) -> bool {
        true
    }
}
