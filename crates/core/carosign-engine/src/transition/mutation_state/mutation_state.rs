use super::LiveMutationConfirmation;
use carosign_core::ids::MutationId;
use core::fmt;

/// Your state of change (registration and deletion).
#[derive(Clone, PartialEq, Eq)]
pub struct MutationState {
    /// Journaled modification.
    pub mutation: MutationId,
    /// A complete, immutable logical plan, if transmitted in a verified manner.
    pub plan: Option<carosign_core::operation::plan::MutationPlan>,
    /// Scheduled registration (document 34).
    ///
    /// ⚠️ Designated with an hardware system; the provider is obliged to repeat it, and only this
    /// echo is received.
    pub planned_new: Option<carosign_core::enrollment::EnrollmentRef>,
    /// The action with the registry selected by the result of the reconciliation (B-01).
    pub registry: Option<crate::reconcile::RegistryAction>,
    /// The result of the reconciliation, on which the action was selected; does not confirm
    /// the record.
    pub reconciliation: Option<crate::reconcile::ReconciliationVerdict>,
    /// Confirmation of the result by a trusted provider in a live session.
    pub live_confirmation: Option<LiveMutationConfirmation>,
}

impl fmt::Debug for MutationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutationState")
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}
