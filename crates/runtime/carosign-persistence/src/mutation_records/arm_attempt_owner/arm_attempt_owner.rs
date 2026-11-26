use super::*;

/// Restrains weapons attempts during the lifetime of the owner, and loss of confirmation or
/// interruption of the call prohibits the repetition of this mutation.
/// The Trusted Constituency is obliged to retain a sole owner and restore closed areas after
/// restart; this table is not a journal.
#[cfg(test)]
pub(crate) struct ArmAttemptOwner {
    attempts: std::sync::Mutex<std::collections::BTreeSet<MutationId>>,
    limit: usize,
}

#[cfg(test)]
impl ArmAttemptOwner {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            attempts: std::sync::Mutex::new(std::collections::BTreeSet::new()),
            limit,
        }
    }

    pub(crate) fn arm(
        &self,
        store: &dyn crate::store::MetadataStore,
        operation: OperationId,
        input: &ArmMutation,
        effect: carosign_engine::PlannedEffect,
        expected_plan: &carosign_core::operation::plan::MutationPlan,
    ) -> Result<crate::journal::ArmedMutationTicket, ArmAttemptFailure> {
        {
            let mut attempts = self
                .attempts
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if attempts.contains(&input.mutation) {
                return Err(ArmAttemptFailure::AlreadyAttempted);
            }
            if attempts.len() >= self.limit {
                return Err(ArmAttemptFailure::Capacity);
            }
            // Recording before accessing the database retains the ban even when interrupted.
            attempts.insert(input.mutation);
        }
        arm_durable_v3(store, operation, input, effect, expected_plan)
            .map_err(ArmAttemptFailure::Durable)
    }
}
