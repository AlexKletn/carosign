/// Initial exchange failure. The damaged state is not reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloError {
    /// Incorrect frame, repetition or mismatch of the expected context.
    Invalid,
    /// Greetings have already been checked or condition closed.
    Closed,
}
