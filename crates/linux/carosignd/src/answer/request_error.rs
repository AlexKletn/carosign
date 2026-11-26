use super::fmt;

/// That's not good enough.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RequestError {
    /// There's no unsub in the shot.
    NoSubject,
    /// There's no authenticator.
    NoAuthenticator,
    /// The unsub is not matched.
    UnknownSubject,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoSubject => "в кадре нет субъекта",
            Self::NoAuthenticator => "в кадре нет аутентификатора",
            Self::UnknownSubject => "субъект не сопоставлен",
        })
    }
}

impl core::error::Error for RequestError {}
