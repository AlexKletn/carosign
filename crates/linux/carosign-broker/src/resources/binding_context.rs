use carosign_core::device::AuthenticatorRef;
use carosign_core::ids::LeaseId;
use carosign_core::ids::WorkerInstanceId;

/// Context of the issue transmitted by the owner of the broker.
/// By itself, it does not confirm a trusted launch or issuance activity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BindingContext {
    pub lease: LeaseId,
    /// The worker to whom the extradition is intended.
    pub worker: WorkerInstanceId,
    /// Authentifier and the age of binding.
    pub authenticator: AuthenticatorRef,
}
