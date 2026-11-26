use carosign_adapters::resource::{BrokerClient, GrantOutcome};
use carosign_core::{ids::LeaseId, resource::ResourceSet};
use carosign_runtime::ports::QuiesceAssessment;

pub(super) struct Malformed(pub(super) GrantOutcome);

impl BrokerClient for Malformed {
    fn acquire(&self, _: &ResourceSet, _: LeaseId) -> GrantOutcome {
        self.0.clone()
    }
    fn release(&self, _: LeaseId) -> QuiesceAssessment {
        panic!("неподтверждённую выдачу нельзя освобождать наугад")
    }
}
