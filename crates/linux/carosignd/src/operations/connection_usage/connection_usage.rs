use super::{BTreeSet, OperationId};

pub(in super::super) struct ConnectionUsage {
    pub(in super::super) limit: usize,
    pub(in super::super) operations: BTreeSet<OperationId>,
    pub(in super::super) detached: bool,
}
