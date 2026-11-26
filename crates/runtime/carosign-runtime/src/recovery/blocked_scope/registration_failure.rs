use super::RecoveryError;
use crate::operation::actor::OperationActor;

/// Refusal of transfer: The actor is returned to the former owner in its entirety.
#[derive(Debug)]
pub struct RegistrationFailure {
    /// Why did the coordinator not accept the operation?
    pub reason: RecoveryError,
    /// Condition and mail of operations that cannot be lost in case of refusal.
    pub actor: Box<OperationActor>,
}
