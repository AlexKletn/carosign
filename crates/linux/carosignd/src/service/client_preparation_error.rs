use crate::accept::AcceptError;

/// Error of conversion or preparation of customer verification.
#[derive(Debug)]
pub enum ClientPreparationError {
    /// A request of another type or incorrect data has been submitted.
    InvalidRequest,
    /// Refusal to prepare.
    Refused(AcceptError),
}
