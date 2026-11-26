use carosign_adapters::resource::{BrokerClient, GrantOutcome};
use carosign_core::{ids::LeaseId, resource::ResourceSet};
use carosign_runtime::ports::QuiesceAssessment;

pub(super) struct Broker;

impl BrokerClient for Broker {
    fn acquire(&self, _set: &ResourceSet, lease: LeaseId) -> GrantOutcome {
        panic!("устаревший посредник не должен получать запрос {lease:?}")
    }
    fn release(&self, _lease: LeaseId) -> QuiesceAssessment {
        QuiesceAssessment::Quiesced
    }
}
