/// Observable outcome: its credibility depends on proven connection and policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    FactorAccepted,
    NotMatched,
    NoSample,
    Cancelled,
    Unavailable,
    Failed,
}
