#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LedgerError {
    Unavailable,
    Corrupt,
    Conflict,
    Invalid,
}
