use crate::idempotency::ReservationError;
use crate::model::StorageError;

#[derive(Debug, PartialEq, Eq)]
pub enum FreshPreparationFailure {
    Storage(StorageError),
    Reservation(ReservationError),
    Existing,
    CommitUnknown(StorageError),
}
