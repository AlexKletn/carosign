use std::sync::{Arc, Weak};

use carosign_core::{identity::SubjectRef, ids::OperationId};
use carosign_runtime::ports::CallerContext;

use crate::cancel_reader::CancellationTarget;

use super::OperationRegistry;

impl OperationRegistry {
    pub(crate) fn register_cancellation(
        &self,
        id: OperationId,
        target: Weak<CancellationTarget>,
    ) -> Option<()> {
        let mut entries = self.entries.lock().ok()?;
        entries.get_mut(&id)?.cancellation = Some(target);
        Some(())
    }

    pub(crate) fn cancellation_for(
        &self,
        id: OperationId,
        caller: &CallerContext,
    ) -> Option<(SubjectRef, bool, Option<Arc<CancellationTarget>>)> {
        let entries = self.entries.lock().ok()?;
        let entry = entries.get(&id)?;
        let owner =
            entry.owner_connection.is_some() && entry.owner == *caller.transaction().as_bytes();
        Some((
            entry.subject,
            owner,
            entry.cancellation.as_ref().and_then(Weak::upgrade),
        ))
    }
}
