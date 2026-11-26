/// Refusal to reserve a place for the result.
#[derive(Debug, PartialEq, Eq)]
pub enum CleanupOwnerError {
    /// The seat's already taken.
    Duplicate,
    /// The table is full.
    Limit,
}
