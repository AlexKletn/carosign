#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StartError {
    Duplicate,
    Limit,
    InvalidDuration,
    Spawn,
}
