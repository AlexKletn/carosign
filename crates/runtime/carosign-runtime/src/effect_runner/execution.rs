use crate::ports::DispatchOutcome;
use carosign_engine::EffectReceipt;

/// What the performer did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Execution {
    /// The effect is complete, here's the observation.
    Receipt(EffectReceipt),
    /// The effect requires storage, but storage is not available.
    StorageUnavailable,
    /// There is no plan to send: the actor did not give it.
    ///
    /// ⚠️ This is not a "sent without a set": without a plan, there is no sending at all.
    NoDispatchPlan,
    /// The effect does nothing on this layer.
    Noop,
    /// The cancellation was not sent or its outcome is unknown; there is no receipt.
    CancelUnconfirmed(DispatchOutcome),
}
