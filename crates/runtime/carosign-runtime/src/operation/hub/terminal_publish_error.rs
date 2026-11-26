use crate::operation::events::EventError;

/// Refusal to publish a sustainable terminal event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerminalPublishError {
    /// The condition does not refer to the completed mutation of this stream.
    ContextMismatch,
    /// A sustainable outcome has not been confirmed.
    Unavailable,
    /// It is impossible to add an event to the stream.
    Event(EventError),
}
