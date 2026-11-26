use super::CycleError;
use crate::operation::bindings::PermittedSet;
use crate::ports::AuthorityRefusal;
use crate::ports::CallerContext;
use crate::ports::CommandPlan;
use crate::ports::PolicyAuthority;
use carosign_core::authorization::GrantScope;

/// Testing one subject on one machine.
///
/// ⚠️ This is a **description of the request, not an authority: the purpose is taken from it,
/// but admission is asked at the source of the authority.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VerificationCycle {
    /// Who asks?
    pub caller: CallerContext,
    /// What it asks (area of permission).
    pub scope: GrantScope,
    /// Set of authorized registrations.
    pub permitted: PermittedSet,
}

impl VerificationCycle {
    /// Collecting a request.
    ///
    /// ⚠️ The set must be non-empty - this is a property of the type `PermittedSet`So you
    /// can’t check it out by eye.
    pub fn new(caller: CallerContext, scope: GrantScope, permitted: PermittedSet) -> Self {
        Self {
            caller,
            scope,
            permitted,
        }
    }

    /// Step 1: Ask for authority.
    ///
    /// ⚠️ Source failure is the end of the cycle, not “try without permission.”
    pub async fn authorize<A: PolicyAuthority>(&self, authority: &A) -> Result<(), CycleError> {
        let GrantScope::Verify { authenticator, .. } = self.scope else {
            return Err(CycleError::Refused(AuthorityRefusal::Denied));
        };
        if self
            .permitted
            .bindings()
            .iter()
            .any(|binding| binding.enrollment.authenticator != authenticator)
        {
            return Err(CycleError::Refused(AuthorityRefusal::Denied));
        }
        let grant = authority
            .authorize(&self.caller, self.scope.clone())
            .await
            .map_err(CycleError::Refused)?;
        if grant.scope() != &self.scope {
            return Err(CycleError::Refused(AuthorityRefusal::Denied));
        }
        Ok(())
    }

    /// Step 2: Check command plan.
    pub fn plan(&self) -> CommandPlan {
        CommandPlan::Verify {
            permitted: self.permitted.clone(),
        }
    }
}
