/// What happened to the test with the eyes of the adapter?
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Factor accepted for this request.
    FactorAccepted,
    NotMatched,
    Cancelled,
    /// Sensor's not available.
    Unavailable,
    /// The service responded with a mistake.
    DaemonError,
    /// The unsub is not matched.
    UnknownSubject,
    /// Proof is not good.
    InvalidEvidence,
    /// It is impossible to determine the outcome reliably.
    Indeterminate,
}
