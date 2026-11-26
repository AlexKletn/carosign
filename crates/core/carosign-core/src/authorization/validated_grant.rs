use super::GrantScope;
use crate::identity::SubjectRef;
use crate::ids::AuthorityId;
use crate::ids::BootId;
use crate::ids::GrantId;
use core::fmt;

/// Confirmed authorization.
///
/// ⚠️ The public designer is not limited to a trusted source, and an object cannot be
/// considered verified based on its type.
#[derive(Clone)]
pub struct ValidatedGrant {
    id: GrantId,
    issuer: AuthorityId,
    scope: GrantScope,
    boot: BootId,
    deadline_ns: u64,
}

impl ValidatedGrant {
    /// Issues a validated permit.
    ///
    /// ⚠️ The public method does not provide trusted provenance; verification of the length
    /// and non-zero value of identifiers is not authorization.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_for_port(
        id: GrantId,
        issuer: AuthorityId,
        scope: GrantScope,
        boot: BootId,
        deadline_ns: u64,
    ) -> Self {
        Self {
            id,
            issuer,
            scope,
            boot,
            deadline_ns,
        }
    }

    /// Permission ID.
    pub fn id(&self) -> GrantId {
        self.id
    }

    /// Who did.
    pub fn issuer(&self) -> AuthorityId {
        self.issuer
    }

    /// That's allowed.
    pub fn scope(&self) -> &GrantScope {
        &self.scope
    }

    /// The download in which it was issued.
    pub fn boot(&self) -> BootId {
        self.boot
    }

    /// Deadline for monotonous loading time, ns.
    pub fn deadline_ns(&self) -> u64 {
        self.deadline_ns
    }

    /// Permission covers the subject.
    pub fn covers(&self, subject: SubjectRef) -> bool {
        self.scope.subject() == Some(subject)
    }
}

impl fmt::Debug for ValidatedGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedGrant")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}
