use carosign_protocol::validation::WireError;
use core::fmt;

/// It's not a good event.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EventError {
    /// The wire field doesn't pass the test.
    Wire(WireError),
    /// Unknown listing.
    Enum {
        field: &'static str,
    },
    NotAnEvent,
    /// Percentages beyond 0..100.
    Percent(u32),
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "провод: {error}"),
            Self::Enum { field } => write!(f, "неизвестное значение {field}"),
            Self::NotAnEvent => f.write_str("кадр не является событием работника"),
            Self::Percent(value) => write!(f, "проценты вне 0..100: {value}"),
        }
    }
}

impl core::error::Error for EventError {}

impl From<WireError> for EventError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}
