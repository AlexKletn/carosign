/// The state of the effect in the queue log.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectPhase {
    /// The effect is planned.
    Planned,
    /// The decision to send is fixed.
    Armed,
    /// Surveillance received.
    Observed,
    /// Exodus requires verification.
    ReconciliationRequired,
}
