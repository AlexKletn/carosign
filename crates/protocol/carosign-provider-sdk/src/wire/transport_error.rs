use carosign_protocol::limits::FRAME_MAX;
use carosign_protocol::validation::WireError;
use core::fmt;

/// What went wrong on the wire.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TransportError {
    /// The frame is longer than the limit.
    TooLarge(usize),
    /// The message's not cleared.
    Malformed,
    /// The message is understood, but its meaning in this phase is incorrect.
    Invalid(WireError),
    /// The frame number is wrong.
    Sequence {
        expected: u64,
        actual: u64,
    },
    /// Canal's closed.
    Closed,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(size) => write!(f, "кадр {size} байт превышает {FRAME_MAX}"),
            Self::Malformed => f.write_str("кадр не разбирается"),
            Self::Invalid(error) => write!(f, "проверка кадра: {error}"),
            Self::Sequence { expected, actual } => {
                write!(f, "номер кадра: ждали {expected}, пришло {actual}")
            }
            Self::Closed => f.write_str("канал закрыт"),
        }
    }
}

impl core::error::Error for TransportError {}

impl From<WireError> for TransportError {
    fn from(value: WireError) -> Self {
        Self::Invalid(value)
    }
}
