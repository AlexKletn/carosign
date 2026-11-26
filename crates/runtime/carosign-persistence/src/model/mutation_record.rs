use core::fmt;

use carosign_core::{
    device::DeviceEpoch,
    enrollment::{EnrollmentGeneration, OpaqueProviderRef},
    identity::SubjectRef,
    ids::{AuthenticatorId, CommandId, MutationId, OperationId, WorkerInstanceId},
};

use crate::{limits, mutation_plan::MutationPlan};

use super::{MutationPhase, MutationTerminalOutcome, RecordError};

/// A change log entry.
#[derive(Clone, PartialEq, Eq)]
pub struct MutationRecord {
    pub mutation: MutationId,
    /// The operation that belongs to it.
    pub operation: OperationId,
    /// Subject and apparatus (region).
    pub subject: SubjectRef,
    pub authenticator: AuthenticatorId,
    /// Expected generation (for registration – target).
    pub expected_generation: Option<EnrollmentGeneration>,
    /// Status.
    pub phase: MutationPhase,
    /// An worker, if you already were.
    pub worker: Option<WorkerInstanceId>,
    /// The age of the device.
    pub epoch: Option<DeviceEpoch>,
    /// Command number, if allowed to be sent.
    pub command: Option<CommandId>,
    /// Recovery link, if the provider has given it.
    pub recovery_ref: Option<OpaqueProviderRef>,
    /// Digest recorded surveillance provider, if any.
    pub provider_observation_digest: Option<[u8; 32]>,
    /// Sustainable outcome, mandatory only in `Completed`.
    pub terminal_outcome: Option<MutationTerminalOutcome>,
    /// Immutable plan of modification (Document 33).
    ///
    /// ⚠️ Mandatory with `Prepared` Recording without a plan. `Corrupt`It is not “nearly
    /// ready”: it can neither be restored nor verified.
    pub plan: Option<MutationPlan>,
}

impl fmt::Debug for MutationRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutationRecord")
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl MutationRecord {
    /// Checking the record form.
    ///
    /// ⚠️ Document 23 allows for empty `worker_instance_id`/`command_id` into `Prepared`The
    /// binding of the sent change is checked upon reading.
    pub fn validate(&self) -> Result<(), RecordError> {
        if matches!(&self.recovery_ref, Some(reference) if reference.len_bytes() > limits::OPAQUE_REF_MAX)
        {
            return Err(RecordError::OpaqueRefTooLong);
        }
        // ⚠️ Document 33: the plan is mandatory with `Prepared` and on.
        let Some(plan) = &self.plan else {
            return Err(RecordError::MissingPlan);
        };
        if let Some(old) = plan.old_target
            && self.expected_generation != Some(old.generation)
        {
            return Err(RecordError::PlanGenerationMismatch);
        }
        Ok(())
    }

    /// Whether sending is allowed only on this record.
    ///
    /// Sustainable recording is not a one-time ticket and does not allow a recovery to repeat
    /// a command after a possible dispatch.
    pub fn allows_dispatch(&self) -> bool {
        false
    }

    /// Area lock key: 48 bytes.
    pub fn scope_key(&self) -> [u8; limits::SCOPE_LOCK_KEY_LEN] {
        let mut key = [0u8; limits::SCOPE_LOCK_KEY_LEN];
        key[..limits::SUBJECT_CANONICAL_LEN].copy_from_slice(&self.subject.to_canonical());
        key[limits::SUBJECT_CANONICAL_LEN..].copy_from_slice(self.authenticator.as_bytes());
        key
    }
}
