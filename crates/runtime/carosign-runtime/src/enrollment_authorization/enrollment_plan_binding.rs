use carosign_core::authorization::{GrantScope, ValidatedGrant};

use super::{EnrollmentAuthorizationError, EnrollmentPlanChange};

/// The immutable identity of the result and the digest of a sustainable plan.
pub struct EnrollmentPlanBinding {
    pub planned: carosign_core::enrollment::EnrollmentRef,
    pub digest: [u8; 32],
    pub change: EnrollmentPlanChange,
}

impl EnrollmentPlanBinding {
    pub fn from_plan(
        plan: &carosign_core::operation::plan::MutationPlan,
        digest: [u8; 32],
        grant: &ValidatedGrant,
    ) -> Result<Self, EnrollmentAuthorizationError> {
        use carosign_core::operation::plan::PlanTransition;
        let (planned, change, old_target) = match plan.transition() {
            PlanTransition::Create { planned_new } => {
                (planned_new, EnrollmentPlanChange::Create, None)
            }
            PlanTransition::Replace { planned_new, old } => (
                planned_new,
                EnrollmentPlanChange::Replace { old },
                Some(old),
            ),
            PlanTransition::Remove { .. } => return Err(EnrollmentAuthorizationError::Invalid),
        };
        if !matches!(grant.scope(), GrantScope::Enroll { subject, authenticator, replace } if *subject == plan.subject() && *authenticator == plan.authenticator() && *replace == old_target)
        {
            return Err(EnrollmentAuthorizationError::Invalid);
        }
        Ok(Self {
            planned,
            digest,
            change,
        })
    }
}
