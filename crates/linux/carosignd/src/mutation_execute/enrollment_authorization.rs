use std::sync::Arc;

use super::MutationStartError;

/// The withdrawal is also valid for cancellation of the future and loss of the owner of the
/// transaction.
pub(super) struct EnrollmentAuthorizationGuard {
    pub(super) authority:
        Arc<dyn carosign_runtime::enrollment_authorization::EnrollmentAuthorizationAuthority>,
    pub(super) scope: carosign_runtime::enrollment_authorization::EnrollmentAuthorizationScope,
    pub(super) authorization:
        Option<carosign_runtime::enrollment_authorization::AuthorizedEnrollment>,
    pub(super) admission:
        Option<Arc<carosign_runtime::enrollment_admission::SerializedEnrollmentAdmission>>,
    pub(super) revoked: bool,
}

impl EnrollmentAuthorizationGuard {
    pub(super) fn revoke(&mut self) -> Result<(), MutationStartError> {
        if !self.revoked {
            if let Some(admission) = &self.admission {
                admission
                    .revoke()
                    .map_err(|_| MutationStartError::Authority)?;
            } else {
                self.authority
                    .revoke_now(&self.scope)
                    .map_err(|_| MutationStartError::Authority)?;
            }
            self.revoked = true;
            self.authorization.take();
        }
        Ok(())
    }
}

impl Drop for EnrollmentAuthorizationGuard {
    fn drop(&mut self) {
        if !self.revoked {
            // The refusal does not confirm cleaning; the fence remains until reconciliation.
            if let Some(admission) = &self.admission {
                let _ = admission.revoke();
            } else {
                let _ = self.authority.revoke_now(&self.scope);
            }
        }
    }
}
