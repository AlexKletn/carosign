#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    Invalid,
    Revoked,
    AlreadyUsed,
    Unavailable,
}
