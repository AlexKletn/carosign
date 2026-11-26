use carosign_core::ids::OperationId;

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdempotentAdmission {
    Created(OperationId),
    Existing(OperationId),
    Conflict,
    Limit,
    Unavailable,
}
