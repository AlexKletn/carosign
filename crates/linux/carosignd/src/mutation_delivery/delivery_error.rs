#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeliveryError {
    /// Sustainable outcome, connection with the recipient or current authority is not proven.
    Unavailable,
    /// The outcome of the socket recording is not confirmed; repetition is prohibited.
    SendUnknown,
}
