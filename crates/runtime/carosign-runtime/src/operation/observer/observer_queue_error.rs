/// Refusal of the outgoing observer queue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObserverQueueError {
    /// The queue is full; the connection must be closed.
    Disconnect,
    /// The event does not continue the sequence of the observer.
    OutOfOrder,
}
