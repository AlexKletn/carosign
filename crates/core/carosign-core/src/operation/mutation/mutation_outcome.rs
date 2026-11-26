use super::MutationFailure;
use crate::enrollment::EnrollmentRef;
use crate::ids::MutationId;

/// How the change ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationOutcome {
    /// Recorded and recorded in the register.
    Committed(EnrollmentRef),
    /// It's been proven that nothing has changed.
    NoChange,
    /// Could have happened; you need a reconciliation.
    NeedsReconciliation(MutationId),
    /// Seems like a denial.
    Failed(MutationFailure),
}

impl MutationOutcome {
    /// Did the outcome leave the area unconfirmed?
    pub fn is_uncertain(&self) -> bool {
        matches!(self, Self::NeedsReconciliation(_))
    }

    /// Can you rewrite the outcome into “nothing has changed”?
    ///
    /// ⚠️ Only in the absence of a proven shipment. `NeedsReconciliation` - Never.
    pub fn may_be_reported_as_no_change(&self) -> bool {
        matches!(self, Self::NoChange)
    }
}
