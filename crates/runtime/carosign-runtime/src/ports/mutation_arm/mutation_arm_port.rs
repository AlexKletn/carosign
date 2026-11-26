use super::super::BoxFuture;
use super::MutationArmFailure;
use super::MutationArmRequest;

/// Separate boundary of sustainable weapons: Implementation is responsible for verifying
/// authority and valid issuance; a returned ticket cannot be copied.
pub trait MutationArmPort: Send + Sync {
    fn arm<'a>(
        &'a self,
        request: &'a MutationArmRequest,
    ) -> BoxFuture<
        'a,
        Result<carosign_persistence::journal::ArmedMutationTicket, MutationArmFailure>,
    >;
}
