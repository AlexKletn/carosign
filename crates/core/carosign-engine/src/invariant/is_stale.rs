use crate::{input::DomainInput, transition::OperationState};

/// The entrance came from a foreign worker or an outdated era.
pub fn is_stale(state: &OperationState, input: &DomainInput) -> bool {
    let DomainInput::DeviceEpochChanged(epoch) = input else {
        return false;
    };
    match state.common.worker {
        Some(worker) => worker.device_epoch != *epoch,
        None => false,
    }
}
