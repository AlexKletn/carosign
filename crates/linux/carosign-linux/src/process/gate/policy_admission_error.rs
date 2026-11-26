#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PolicyAdmissionError {
    Closed,
    StaleGeneration,
    Limit,
    Unknown,
    Io,
}
