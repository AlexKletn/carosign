use super::{FdPlanError, Handoff, Revocation};

use crate::resources::{Binding, ClaimError, FdRole};

/// Outcome of extradition verification.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HandoffPlan {
    /// What to do.
    pub action: Handoff,
    /// How reliable is the delivery after the recoil?
    pub revocation: Revocation,
}

impl HandoffPlan {
    /// It does.
    pub fn deliver() -> Self {
        Self {
            action: Handoff::Deliver,
            revocation: Revocation::OnlyByTermination,
        }
    }

    /// Refusal.
    pub fn reject() -> Self {
        Self {
            action: Handoff::RejectAndCloseAll,
            revocation: Revocation::NotApplicable,
        }
    }
}

/// Checking the delivery plan.
///
/// ⚠️ The output does not store file system paths: only identifiers and roles.
pub fn plan(claims: &[Binding], expected_roles: &[FdRole]) -> Result<HandoffPlan, FdPlanError> {
    if claims.len() != expected_roles.len() {
        return Err(FdPlanError::CountMismatch {
            declared: expected_roles.len(),
            received: claims.len(),
        });
    }
    // Form confirmation does not prove trust, but a single parcel cannot combine descriptors
    // of different outputs, workers or devices.
    if let Some(first) = claims.first() {
        for (index, binding) in claims.iter().enumerate() {
            if binding.epoch != first.epoch {
                return Err(FdPlanError::Claim(ClaimError::StaleEpoch));
            }
            if binding.lease != first.lease
                || binding.worker != first.worker
                || binding.authenticator != first.authenticator
            {
                return Err(FdPlanError::Claim(ClaimError::NotInLease {
                    resource: binding.resource,
                }));
            }
            if claims[..index]
                .iter()
                .any(|previous| previous.resource == binding.resource)
            {
                return Err(FdPlanError::Claim(ClaimError::Duplicate {
                    resource: binding.resource,
                }));
            }
        }
    }
    // Roles must go in the same order in which slots are declared.
    for (binding, role) in claims.iter().zip(expected_roles) {
        if binding.role != *role {
            return Err(FdPlanError::RoleMismatch);
        }
    }
    Ok(HandoffPlan::deliver())
}
