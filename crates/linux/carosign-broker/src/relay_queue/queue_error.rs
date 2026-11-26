#[derive(Debug, PartialEq, Eq)]
pub(crate) enum QueueError {
    Busy,
    Closed,
    InvalidFrame,
}
