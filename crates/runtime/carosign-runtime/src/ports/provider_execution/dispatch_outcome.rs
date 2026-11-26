/// How did the shipment end?
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DispatchOutcome {
    /// Accepted by the provider (does not mean executed).
    Accepted,
    /// Confirmed not transferred.
    NotSent,
    /// I don't know if I got it.
    Unknown,
    /// Wire failure.
    Failed,
}
