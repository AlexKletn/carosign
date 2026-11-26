/// Resource Owner Context: The form itself does not prove trusted startup or authority; these
/// facts confirm the makeup of the platform.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResourceOwner {
    pub operation: carosign_core::ids::OperationId,
    /// Copy of the worker.
    pub worker: carosign_core::ids::WorkerInstanceId,
    /// The authenticator and the era of its binding.
    pub authenticator: carosign_core::device::AuthenticatorRef,
}
