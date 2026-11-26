use crate::model::StorageError;

use carosign_core::{
    identity::SubjectRef,
    ids::{AuthenticatorId, MutationId, OperationId},
};

/// The input of the change preparation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrepareMutation {
    pub mutation: MutationId,
    pub operation: OperationId,
    /// Area.
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
    /// Expected generation (for replacement and removal).
    pub expected_generation: Option<carosign_core::enrollment::EnrollmentGeneration>,
    /// Whether it's registration.
    pub enroll: bool,
    /// Immutable plan of modification (Document 33).
    ///
    /// ⚠️ Without it, recovery doesn’t know what it was supposed to be.
    pub plan: crate::mutation_plan::MutationPlan,
}

/// Check that the journal can accept such a record.
pub fn validate_prepare(input: &PrepareMutation) -> Result<(), StorageError> {
    input
        .plan
        .validate(crate::mutation_plan::Scope {
            subject: input.subject,
            authenticator: input.authenticator,
        })
        .map_err(|_| StorageError::Corrupt {
            what: "план не соответствует области",
        })?;
    if input.enroll != input.plan.transition.creates_enrollment() {
        return Err(StorageError::Corrupt {
            what: "вид изменения не соответствует плану",
        });
    }
    if let Some(old) = input.plan.old_target
        && input.expected_generation != Some(old.generation)
    {
        return Err(StorageError::GenerationConflict);
    }
    Ok(())
}
