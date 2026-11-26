use super::ObservationFailure;

/// Refusal to prepare the result to the original owner.
#[derive(Debug, PartialEq, Eq)]
pub enum OwnerDeliveryFailure {
    /// The actor is unavailable, busy or uncompleted safely.
    Operation(ObservationFailure),
    /// The recipient does not match the original request.
    WrongOwner,
    /// The source of authority refused before extradition.
    Authority(carosign_runtime::ports::AuthorityRefusal),
    /// The result has already been transferred to the owner of the issue or reserved for him.
    AlreadyReserved,
    /// Confirmed match delivery is not yet connected.
    EvidenceUnavailable,
    /// The transaction flow cannot give the next event number.
    Events(carosign_runtime::operation::events::EventError),
}
