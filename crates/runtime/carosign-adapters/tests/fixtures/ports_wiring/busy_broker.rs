use carosign_adapters::resource::{BrokerClient, GrantOutcome};
use carosign_core::{ids::LeaseId, resource::ResourceSet};
use carosign_runtime::ports::QuiesceAssessment;

pub(super) struct BusyBroker;

impl BrokerClient for BusyBroker {
    fn acquire(&self, _set: &ResourceSet, _lease: LeaseId) -> GrantOutcome {
        panic!("устаревший посредник не должен получать запрос")
    }
    fn release(&self, _lease: LeaseId) -> QuiesceAssessment {
        QuiesceAssessment::Uncertain
    }
}
