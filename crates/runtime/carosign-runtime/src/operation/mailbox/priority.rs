/// Category of message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Priority {
    /// Review, deadline, cancellation, loss of worker, change of era.
    Control,
    /// Confirmations, progress, completions.
    Normal,
}
