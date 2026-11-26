use core::fmt;

/// Flow failure.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EventError {
    /// The next number is held by the owner of the terminal result.
    Reserved,
    /// The last valid number has already been issued.
    SequenceExhausted,
    /// The result of this stream has already been published.
    AlreadyTerminal,
    /// The numbers went out of order.
    OutOfOrder {
        expected: u64,
        actual: u64,
    },
}

impl fmt::Debug for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved => f.write_str("EventError::Reserved"),
            Self::SequenceExhausted => f.write_str("EventError::SequenceExhausted"),
            Self::AlreadyTerminal => f.write_str("EventError::AlreadyTerminal"),
            Self::OutOfOrder { expected, actual } => {
                write!(f, "EventError::OutOfOrder({expected} != {actual})")
            }
        }
    }
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved => f.write_str("номер события зарезервирован"),
            Self::SequenceExhausted => f.write_str("номера событий исчерпаны"),
            Self::AlreadyTerminal => f.write_str("терминальное событие уже опубликовано"),
            Self::OutOfOrder { expected, actual } => {
                write!(f, "номер события: ждали {expected}, пришло {actual}")
            }
        }
    }
}

impl core::error::Error for EventError {}
