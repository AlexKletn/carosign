use super::MutationId;

/// Proven association of an incomplete mutation with the enclosed area.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BlockedMutationScope {
    /// An incomplete change.
    pub mutation: MutationId,
    /// The subject of the change.
    pub subject: carosign_core::identity::SubjectRef,
    /// Change authenticator.
    pub authenticator: carosign_core::ids::AuthenticatorId,
}
