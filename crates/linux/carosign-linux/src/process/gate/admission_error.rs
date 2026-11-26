#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    StaleGeneration,
    Closed,
    Io,
}
