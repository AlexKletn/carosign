#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChallengeError {
    Expired,
    Duplicate,
    Full,
    Unknown,
    Mismatch,
    Consumed,
    Unavailable,
}
