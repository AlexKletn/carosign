#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MutationPrepareError {
    Invalid,
    Denied,
    Unavailable,
    Conflict,
}
