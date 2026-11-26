use crate::codec::RequestError;
use crate::frame;
use core::fmt;

/// Session failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SessionError {
    /// The message is not a greeting, and there is no greeting.
    HelloExpected,
    /// Second hello.
    AlreadyHelloed,
    /// The version doesn't agree.
    VersionMismatch,
    /// The request is not being handled.
    Request(RequestError),
    /// It's not a good shot.
    Frame(frame::FrameError),
    /// More connection operations.
    TooManyOperations,
    /// Beyond the greeting period.
    HelloTimeout,
    /// Session's closed.
    Closed,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HelloExpected => f.write_str("первое сообщение должно быть приветствием"),
            Self::AlreadyHelloed => f.write_str("приветствие уже было"),
            Self::VersionMismatch => f.write_str("версия не согласуется"),
            Self::Request(error) => write!(f, "запрос: {error}"),
            Self::Frame(error) => write!(f, "кадр: {error}"),
            Self::TooManyOperations => f.write_str("операций на соединение сверх предела"),
            Self::HelloTimeout => f.write_str("приветствие не пришло в срок"),
            Self::Closed => f.write_str("сеанс закрыт"),
        }
    }
}

impl core::error::Error for SessionError {}
