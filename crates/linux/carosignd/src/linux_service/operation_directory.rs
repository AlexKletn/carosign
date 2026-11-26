use std::sync::{Arc, Mutex, Weak};

use carosign_core::ids::OperationId;

use crate::operations::{LiveOperationStatus, OperationRegistry};

#[derive(Default)]
pub(super) struct OperationDirectory(Mutex<Vec<Weak<OperationRegistry>>>);

impl OperationDirectory {
    pub(super) fn add(&self, registry: &Arc<OperationRegistry>) {
        let mut registries = self.0.lock().unwrap_or_else(|error| error.into_inner());
        registries.retain(|entry| entry.strong_count() != 0);
        registries.push(Arc::downgrade(registry));
    }

    pub(super) fn registry(&self, operation: OperationId) -> Option<Arc<OperationRegistry>> {
        self.registries()
            .into_iter()
            .find(|registry| registry.event_hub(operation).is_ok())
    }

    pub(super) fn find(&self, operation: OperationId) -> Option<LiveOperationStatus> {
        self.registries()
            .into_iter()
            .find_map(|registry| registry.live_status(operation))
    }

    fn registries(&self) -> Vec<Arc<OperationRegistry>> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }
}
