/// Receiver Call Outcome: No option allows re-sending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumptionOutcome {
    Completed,
    NotDispatched,
    Unknown,
}
