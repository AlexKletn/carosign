/// The result of two consecutive cleaning steps.
#[derive(Debug)]
pub enum CleanupProgress {
    /// The Trusted Broker confirmed the peace and release of the same extradition.
    Confirmed(carosign_runtime::operation::actor::Step),
    /// Security is unknown; the lease is retained at the operation.
    Uncertain(carosign_runtime::operation::actor::Step),
}
