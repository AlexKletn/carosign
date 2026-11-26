use std::collections::BTreeSet;

use carosign_core::ids::{OperationId, ResourceId};

pub struct FenceInventory {
    pub resources: Vec<ResourceId>,
    pub operations: Option<BTreeSet<OperationId>>,
}
