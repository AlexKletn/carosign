#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueryError {
    Unavailable,
    Unauthorized,
    Unexpected,
}

impl QueryError {
    pub(crate) fn message_key(self) -> &'static str {
        match self {
            Self::Unavailable => "monitor-unavailable",
            Self::Unauthorized => "ui-access-denied",
            Self::Unexpected => "monitor-unexpected",
        }
    }
}
