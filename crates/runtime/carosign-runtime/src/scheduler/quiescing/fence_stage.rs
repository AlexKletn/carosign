/// Stage of fencing withheld extradition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenceStage {
    /// Confirmation of the broker is pending; new issuances are blocked.
    Quiescing,
    /// The result is unknown; the fence is maintained.
    Uncertain,
}
