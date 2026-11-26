use super::IdempotencyRecord;

/// Results of verification in the current transaction.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reservation {
    /// A new recording has been prepared, but has not yet been recorded on the disc.
    Staged,
    /// The same request already has an operation; a new one cannot be started.
    Existing(IdempotencyRecord),
}
