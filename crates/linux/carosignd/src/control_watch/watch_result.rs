use carosign_runtime::operation::actor::Step;

use crate::operations::ObservationFailure;

/// The result is maintained regardless of the owner’s receipt of the flow.
pub(crate) enum WatchResult {
    Step(Step),
    /// Management is kept in the register for the employed actor.
    Deferred,
    /// The source or actor refused; it's not confirmation of the cleanup.
    Failed(ObservationFailure),
}
