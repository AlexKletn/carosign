use carosign_protocol::validation::WireError;
use core::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RequestError {
    Wire(WireError),
    Malformed,
    NotARequest,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "провод: {error}"),
            Self::Malformed => f.write_str("кадр не разбирается"),
            Self::NotARequest => f.write_str("сообщение не является запросом"),
        }
    }
}

impl core::error::Error for RequestError {}

impl From<WireError> for RequestError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}
