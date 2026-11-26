/// Final enrollment outcome. Success requires a durable commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnrollOutcome {
    Succeeded,
    Failed,
    Duplicate,
    DataFull,
    Disconnected,
    UnknownError,
}

impl EnrollOutcome {
    pub(super) fn status(self) -> &'static str {
        match self {
            Self::Succeeded => "enroll-completed",
            Self::Failed => "enroll-failed",
            Self::Duplicate => "enroll-duplicate",
            Self::DataFull => "enroll-data-full",
            Self::Disconnected => "enroll-disconnected",
            Self::UnknownError => "enroll-unknown-error",
        }
    }
}
