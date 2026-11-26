use carosign_core::{ids::OperationId, operation::context::BootInstant};

use crate::{answer::Reply, run::RunOutcome};

/// How did the request end?
#[derive(Clone, PartialEq, Debug)]
pub enum Served {
    /// Operation accepted, steps taken.
    Ran {
        /// Number.
        operation: OperationId,
        /// Operation time (loading number and moment).
        deadline: BootInstant,
        /// How the steps ended.
        outcome: RunOutcome,
    },
    /// Denial before surgery.
    Refused(Reply),
}
