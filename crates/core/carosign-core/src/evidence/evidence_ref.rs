use crate::ids::EvidenceId;
use crate::ids::OperationId;
use core::fmt;

/// Reference to withheld evidence.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EvidenceRef {
    id: EvidenceId,
    operation: OperationId,
}

impl EvidenceRef {
    /// Reference to proof of this* operation.
    pub fn new(id: EvidenceId, operation: OperationId) -> Self {
        Self { id, operation }
    }

    /// The evidence ID itself.
    pub fn id(&self) -> EvidenceId {
        self.id
    }

    /// What kind of operation is it tied to?
    pub fn operation(&self) -> OperationId {
        self.operation
    }

    /// Whether suitable for surgery: someone else's link is not overplayed.
    pub fn belongs_to(&self, operation: OperationId) -> bool {
        self.operation == operation
    }
}

impl fmt::Debug for EvidenceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvidenceRef(..)")
    }
}
