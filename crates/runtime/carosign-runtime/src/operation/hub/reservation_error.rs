use crate::operation::events::EventError;

/// Reason for the reserve failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReservationError {
    /// Flow, actor, effect, or stage are not the same.
    ContextMismatch,
    /// The reserve is held or the outcome is unknown.
    Busy,
    /// The numbers are exhausted or the stream is already complete.
    Event(EventError),
}
