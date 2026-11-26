use super::StorageError;

/// Refusal to prepare the recording.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReservationError {
    /// The same key has already been used for another request.
    Conflict,
    /// The storage or record is incorrect.
    Storage(StorageError),
}
