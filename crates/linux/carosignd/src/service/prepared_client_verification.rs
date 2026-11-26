use carosign_runtime::{VerificationCycle, ports::CallerContext};

/// Preparation of the client's audit. The mere presence of training does not trigger the
/// operation: first, you need to take into account the atomic key of idempotence.
pub struct PreparedClientVerification {
    /// Calling, received from the transport.
    pub caller: CallerContext,
    /// Request key.
    pub idempotency: carosign_core::ids::IdempotencyKey,
    /// Original unverified intent.
    pub intent: carosign_runtime::authority::verification::VerificationIntent,
    /// A cycle with the ultimate goal of politics.
    pub cycle: VerificationCycle,
    /// Permission granted to the original intent.
    pub grant: carosign_core::authorization::ValidatedGrant,
}
