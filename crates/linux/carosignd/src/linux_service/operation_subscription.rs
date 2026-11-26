use std::sync::{Arc, Mutex};

use carosign_runtime::operation::{
    events::Event,
    hub::{AttachError, EventHub},
};

pub(super) struct OperationSubscription {
    hub: Arc<Mutex<EventHub>>,
    id: u32,
    generation: u64,
}

impl OperationSubscription {
    pub(super) fn attach(
        hub: Arc<Mutex<EventHub>>,
        snapshot_sequence: u64,
        after_sequence: u64,
        limit: usize,
    ) -> Result<(Self, Option<Vec<Event>>), AttachError> {
        let bytes =
            carosign_linux::sys::random_16().map_err(|_| AttachError::GenerationExhausted)?;
        let id = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let mut locked = hub.lock().map_err(|_| AttachError::GenerationExhausted)?;
        let replay = locked.stream().retained_after(after_sequence);
        locked.attach_with_limit(id, snapshot_sequence, limit)?;
        let generation = locked
            .generation(id)
            .ok_or(AttachError::GenerationExhausted)?;
        drop(locked);
        Ok((
            Self {
                hub,
                id,
                generation,
            },
            replay,
        ))
    }

    pub(super) fn pop(&self) -> Result<Option<Event>, ()> {
        let mut hub = self.hub.lock().map_err(|_| ())?;
        if hub.generation(self.id) != Some(self.generation) {
            return Err(());
        }
        Ok(hub.pop(self.id))
    }
}

impl Drop for OperationSubscription {
    fn drop(&mut self) {
        if let Ok(mut hub) = self.hub.lock()
            && hub.generation(self.id) == Some(self.generation)
        {
            hub.detach(self.id);
        }
    }
}

#[cfg(test)]
#[path = "operation_subscription.tests.rs"]
mod tests;
