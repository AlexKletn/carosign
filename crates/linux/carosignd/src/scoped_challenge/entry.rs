use super::{OperationId, ScopedAssertionContext};

pub(super) struct Entry {
    pub(super) operation: OperationId,
    pub(super) context: ScopedAssertionContext,
    pub(super) consumed: bool,
}
