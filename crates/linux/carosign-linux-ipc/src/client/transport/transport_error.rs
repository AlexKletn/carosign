/// Transportation error or inadmissible response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    Unavailable,
    Unexpected,
}
