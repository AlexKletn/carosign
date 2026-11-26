use carosign_core::ids::MutationId;
use carosign_core::ids::OperationId;

/// Context of change, known operations.
///
/// ⚠️ It's just what the journal has to write down: the subject, the apparatus, and the
/// expected generation, and the change number comes in separately, and the owner of the
/// operation issues it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MutationIntent {
    pub mutation: MutationId,
    pub operation: OperationId,
    pub subject: carosign_core::identity::SubjectRef,
    pub authenticator: carosign_core::ids::AuthenticatorId,
    pub expected_generation: Option<carosign_core::enrollment::EnrollmentGeneration>,
    /// If it's a registration.
    pub enroll: bool,
    /// Immutable plan of modification (Document 33).
    pub plan: carosign_persistence::mutation_plan::MutationPlan,
}
