#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureStatus {
    Retry,
    SwipeTooShort,
    TooFast,
    FingerNotCentered,
    RemoveAndRetry,
}

impl CaptureStatus {
    pub(super) fn verify(self) -> &'static str {
        match self {
            Self::Retry => "verify-retry-scan",
            Self::SwipeTooShort => "verify-swipe-too-short",
            Self::TooFast => "verify-too-fast",
            Self::FingerNotCentered => "verify-finger-not-centered",
            Self::RemoveAndRetry => "verify-remove-and-retry",
        }
    }

    pub(super) fn enroll(self) -> &'static str {
        match self {
            Self::Retry => "enroll-retry-scan",
            Self::SwipeTooShort => "enroll-swipe-too-short",
            Self::TooFast => "enroll-too-fast",
            Self::FingerNotCentered => "enroll-finger-not-centered",
            Self::RemoveAndRetry => "enroll-remove-and-retry",
        }
    }
}
